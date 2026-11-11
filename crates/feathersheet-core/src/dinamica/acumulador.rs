//! Agregador incremental: não armazena um f64 por linha quando basta
//! acumular soma, contagem, mínimo, máximo ou média.
//! Moda conserva somente frequências por valor; o desvio-padrão amostral
//! usa Welford (média e M2), com memória O(1) mesmo em milhões de linhas.
use std::collections::HashMap;
use super::pivot::AgregacaoOp;
use crate::celulas::values::{to_number, Value};

/// O que uma célula da origem leva para a medida, como no Excel: vazio não
/// entra em conta nenhuma; texto que não é número entra só na Contagem.
/// Antes os dois viravam 0 e puxavam Média, DesvPad e Mín para baixo
/// (17/10/2026, análise de 13,7 milhões de linhas do usuário).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Medida {
    Vazio,
    /// O hash do texto em minúsculas (para Distintos).
    Texto(u64),
    Numero(f64),
}

fn hash_texto(t: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for c in t.chars().flat_map(char::to_lowercase) { c.hash(&mut h); }
    h.finish()
}

/// O percentil `p` (0 a 1) com interpolação, como o PERCENTIL.INC do Excel.
pub(crate) fn percentil(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() { return 0.0; }
    v.sort_by(f64::total_cmp);
    let pos = p * (v.len() - 1) as f64;
    let (i, frac) = (pos.floor() as usize, pos - pos.floor());
    match v.get(i + 1) {
        Some(prox) if frac > 0.0 => v[i] + frac * (prox - v[i]),
        _ => v[i],
    }
}

impl Medida {
    pub(crate) fn de(v: &Value) -> Self {
        match v {
            Value::Number(n) => Medida::Numero(*n),
            Value::Empty => Medida::Vazio,
            Value::Text(t) if t.trim().is_empty() => Medida::Vazio,
            outro => match to_number(outro) {
                Ok(n) => Medida::Numero(n),
                Err(_) => Medida::Texto(match outro { Value::Text(t) => hash_texto(t), _ => 0 }),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Acumulador {
    op: AgregacaoOp,
    quantidade: usize,
    soma: f64,
    menor: f64,
    maior: f64,
    // Algoritmo de Welford: reduz cancelamento numérico sem reter entradas.
    media_desvio: f64,
    soma_quadrados: f64,
    frequencias: HashMap<u64, usize>,
    moda: f64,
    moda_frequencia: usize,
    /// Distintos: bits do número, ou o hash do texto com o bit alto trocado.
    distintos: std::collections::HashSet<u64>,
    /// Mediana e percentil: todos os números do grupo.
    numeros: Vec<f64>,
}

impl Acumulador {
    pub fn novo(op: AgregacaoOp) -> Self {
        Self {
            op, quantidade: 0, soma: 0., menor: f64::INFINITY,
            maior: f64::NEG_INFINITY, media_desvio: 0.0, soma_quadrados: 0.0,
            frequencias: HashMap::new(), moda: 0., moda_frequencia: 0,
            distintos: Default::default(), numeros: Vec::new(),
        }
    }
    /// Vazio fica de fora; texto só soma na Contagem.
    pub fn receber_medida(&mut self, m: Medida) {
        match m {
            Medida::Numero(n) => self.receber(n),
            Medida::Texto(_) if self.op == AgregacaoOp::Contagem => self.quantidade += 1,
            // O texto entra num espaço de chaves à parte do dos números (`!h`... basta um xor).
            Medida::Texto(h) if self.op == AgregacaoOp::Distintos => {
                self.quantidade += 1;
                self.distintos.insert(h ^ 0x8000_0000_0000_0001);
            }
            _ => {}
        }
    }
    pub fn receber(&mut self, n: f64) {
        self.quantidade += 1;
        match self.op {
            AgregacaoOp::Soma | AgregacaoOp::Media => self.soma += n,
            AgregacaoOp::DesvioPadrao => {
                let diferenca = n - self.media_desvio;
                self.media_desvio += diferenca / self.quantidade as f64;
                let nova_diferenca = n - self.media_desvio;
                self.soma_quadrados += diferenca * nova_diferenca;
            }
            AgregacaoOp::Min => self.menor = self.menor.min(n),
            AgregacaoOp::Max => self.maior = self.maior.max(n),
            AgregacaoOp::Moda => {
                let frequencia = self.frequencias.entry(n.to_bits()).or_insert(0);
                *frequencia += 1;
                if *frequencia > self.moda_frequencia {
                    self.moda_frequencia = *frequencia;
                    self.moda = n;
                }
            }
            AgregacaoOp::Distintos => { self.distintos.insert(if n == 0.0 { 0 } else { n.to_bits() }); }
            AgregacaoOp::Mediana | AgregacaoOp::Percentil(_) => self.numeros.push(n),
            AgregacaoOp::Contagem => {}
        }
    }
    pub fn resultado(&self) -> f64 {
        if self.quantidade == 0 { return 0.; }
        match self.op {
            AgregacaoOp::Soma => self.soma,
            AgregacaoOp::Contagem => self.quantidade as f64,
            AgregacaoOp::Media => self.soma / self.quantidade as f64,
            AgregacaoOp::Min => self.menor,
            AgregacaoOp::Max => self.maior,
            AgregacaoOp::Moda => self.moda,
            AgregacaoOp::Distintos => self.distintos.len() as f64,
            AgregacaoOp::Mediana => percentil(&mut self.numeros.clone(), 0.5),
            AgregacaoOp::Percentil(p) => percentil(&mut self.numeros.clone(), p as f64 / 100.0),
            AgregacaoOp::DesvioPadrao => {
                if self.quantidade < 2 { return 0.; }
                (self.soma_quadrados.max(0.) / (self.quantidade - 1) as f64).sqrt()
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use super::super::pivot::aggregate;
    #[test]
    fn acumulador_igual_ao_motor_legado_em_todas_as_operacoes() {
        let numeros = [3., 5., 3., 2., 5., 5., 1.];
        for op in [
            AgregacaoOp::Soma, AgregacaoOp::Contagem, AgregacaoOp::Media,
            AgregacaoOp::Min, AgregacaoOp::Max, AgregacaoOp::DesvioPadrao,
            AgregacaoOp::Moda, AgregacaoOp::Distintos, AgregacaoOp::Mediana, AgregacaoOp::Percentil(90),
        ] {
            let mut acc = Acumulador::novo(op);
            for &n in &numeros { acc.receber(n); }
            let esperado = aggregate(&numeros, op);
            assert!((acc.resultado() - esperado).abs() < 1e-10,
                "{op:?}: {} versus {esperado}", acc.resultado());
        }
    }
    #[test]
    fn desvio_amostral_em_milhao_de_linhas_usa_memoria_constante() {
        let mut a = Acumulador::novo(AgregacaoOp::DesvioPadrao);
        for i in 0..1_000_000 {
            a.receber((i % 2) as f64);
        }
        let esperado = ((1_000_000.0_f64 / 999_999.0) * 0.25).sqrt();
        assert!((a.resultado() - esperado).abs() < 1e-8);
        assert!(a.frequencias.is_empty());
    }

    #[test]
    fn vazio_fica_de_fora_e_texto_so_conta() {
        let entradas = [
            Value::Number(2.), Value::Empty, Value::Text("abc".into()),
            Value::Number(4.), Value::Text("  ".into()), Value::Text("6".into()),
        ];
        let resultado = |op| {
            let mut acc = Acumulador::novo(op);
            for v in &entradas { acc.receber_medida(Medida::de(v)); }
            acc.resultado()
        };
        assert_eq!(resultado(AgregacaoOp::Soma), 12.);
        assert_eq!(resultado(AgregacaoOp::Media), 4.);
        assert_eq!(resultado(AgregacaoOp::Min), 2.);
        assert_eq!(resultado(AgregacaoOp::Contagem), 4.);
        assert!((resultado(AgregacaoOp::DesvioPadrao) - 2.).abs() < 1e-12);
    }

    #[test]
    fn milhao_de_linhas_na_soma_nao_aloca_vetor_de_valores() {
        let mut acc = Acumulador::novo(AgregacaoOp::Soma);
        for _ in 0..1_000_000 { acc.receber(2.); }
        assert_eq!(acc.resultado(), 2_000_000.);
        assert!(acc.frequencias.is_empty());
    }

    #[test]
    fn distintos_mediana_e_percentil_como_no_excel() {
        let mut d = Acumulador::novo(AgregacaoOp::Distintos);
        for v in [Value::Text("Sul".into()), Value::Text("SUL".into()), Value::Number(3.), Value::Number(3.),
                  Value::Empty, Value::Text("Norte".into()), Value::Number(-0.0), Value::Number(0.)] {
            d.receber_medida(Medida::de(&v));
        }
        assert_eq!(d.resultado(), 4.0, "sul, 3, norte, 0");
        // PERCENTIL.INC({1;2;3;4};0,9) = 3,7; MED({1;2;3;4}) = 2,5; texto e vazio ficam de fora.
        for (op, esperado) in [(AgregacaoOp::Mediana, 2.5), (AgregacaoOp::Percentil(90), 3.7), (AgregacaoOp::Percentil(25), 1.75)] {
            let mut a = Acumulador::novo(op);
            for v in [Value::Number(4.), Value::Text("x".into()), Value::Number(1.), Value::Empty, Value::Number(3.), Value::Number(2.)] {
                a.receber_medida(Medida::de(&v));
            }
            assert!((a.resultado() - esperado).abs() < 1e-12, "{op:?}: {}", a.resultado());
        }
    }
}
