//! Agregado por critério: uma passada pelas colunas serve a todas as
//! `SOMASES`/`CONT.SES`/`MÉDIASES` que leem as mesmas faixas.
//!
//! A aba Resumo da Planilha Pesada tem ~150 `SOMASES(Dados!I; Dados!C; $A2;
//! Dados!J; 1)`: só muda o critério (`$A2`). Cada uma varria 1 milhão de
//! linhas — ~25 ms, 4,4 s a aba inteira, mesmo com a faixa já lida de uma
//! vez ([`crate::planilha::faixa_lida`]). Aqui a varredura acontece **uma
//! vez por conjunto de faixas**: cada linha é posta no grupo da tupla dos
//! seus valores de critério, e o grupo acumula soma, contagem, máximo e
//! mínimo da faixa somada. Depois cada fórmula é uma consulta ao mapa.
//!
//! Só critério de **igualdade simples** usa o agregado (número diferente de
//! zero, ou texto sem operador e sem curinga) — é o que
//! [`crate::funcoes::fn_helpers::make_predicate`] trata como "igualdade", com
//! a mesma regra de comparação (texto sem ligar para maiúscula, número por
//! valor). `">10"`, `"a*"`, vazio, zero e lógico continuam no caminho célula
//! por célula, que já estava certo.
//!
//! Frente de velocidade V2 do plano (`docs/planejamento/PLANO-MEMORIA-400MB.md`).

use std::collections::HashMap;
use std::sync::Arc;

use crate::celulas::values::Value;
use crate::planilha::data_table::Cru;

/// Abaixo disto a varredura direta já é barata; não vale montar o mapa.
pub(crate) const MINIMO: u64 = 2_048;
/// Mais grupos que isto é uma coluna de valores quase únicos: o mapa
/// ficaria do tamanho da coluna sem servir a ninguém.
pub(crate) const MAX_GRUPOS: usize = 262_144;

/// O valor de um critério (ou de uma célula) na forma em que se compara.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ChaveCriterio {
    /// Bits do `f64`, com `-0` normalizado para `0`.
    Numero(u64),
    /// Texto em minúsculas (a igualdade de critério ignora maiúsculas).
    /// `Arc` para a chave de uma célula custar um incremento, não uma cópia.
    Texto(Arc<str>),
}

fn bits(n: f64) -> u64 {
    if n == 0.0 { 0.0f64.to_bits() } else { n.to_bits() }
}

fn minusculas(s: &str) -> Arc<str> {
    if s.is_ascii() {
        Arc::from(s.to_ascii_lowercase())
    } else {
        Arc::from(s.chars().flat_map(char::to_lowercase).collect::<String>())
    }
}

/// Lê a chave de cada célula de critério e dá a cada chave distinta um
/// código pequeno (`u32`): o agregado agrupa por códigos, não por textos.
/// Quem monta o agregado implementa sobre o armazenamento da aba (`Sheet`),
/// lendo o texto pelo id do pool quando dá — a mesma string de 1 milhão de
/// linhas passa para minúscula uma vez, não um milhão. Com textos, eram
/// ~240 ns por linha (13,7 milhões de linhas: 3,3 s, 17/10/2026): um `Arc`
/// clonado e o texto inteiro hasheado de novo por critério.
pub(crate) struct LeitorChaves<'a> {
    /// Código por id de texto no pool: [`NAO_VISTO`], [`NAO_CASA`] ou o código.
    por_id: Vec<u32>,
    /// Os mesmos, separados para a célula lógica (`literal`), que não casa.
    por_id_literal: Vec<u32>,
    por_numero: HashMap<u64, u32, std::hash::BuildHasherDefault<crate::celulas::mapa_chave::HasherChave>>,
    /// Texto como está na célula (sem passar para minúscula) → código.
    por_texto: HashMap<Box<str>, u32>,
    codigos: HashMap<ChaveCriterio, u32>,
    /// A chave de cada código.
    pub chaves: Vec<ChaveCriterio>,
    pub texto_do_pool: &'a dyn Fn(i32) -> &'a str,
}

const NAO_VISTO: u32 = u32::MAX;
const NAO_CASA: u32 = u32::MAX - 1;

impl<'a> LeitorChaves<'a> {
    pub(crate) fn novo(texto_do_pool: &'a dyn Fn(i32) -> &'a str) -> Self {
        LeitorChaves {
            por_id: Vec::new(), por_id_literal: Vec::new(), por_numero: Default::default(),
            por_texto: HashMap::new(), codigos: HashMap::new(), chaves: Vec::new(), texto_do_pool,
        }
    }

    /// O código de uma chave.
    pub(crate) fn codigo(&mut self, chave: ChaveCriterio) -> u32 {
        if let Some(&c) = self.codigos.get(&chave) {
            return c;
        }
        let c = self.chaves.len() as u32;
        self.chaves.push(chave.clone());
        self.codigos.insert(chave, c);
        c
    }

    fn numero(&mut self, n: f64) -> Result<Option<u32>, ()> {
        if n.is_nan() {
            return Err(());
        }
        let b = bits(n);
        if let Some(&c) = self.por_numero.get(&b) {
            return Ok(Some(c));
        }
        let c = self.codigo(ChaveCriterio::Numero(b));
        self.por_numero.insert(b, c);
        Ok(Some(c))
    }

    /// O código de um valor cru (número ou id de texto no pool).
    /// `literal`: o cru veio da coluna de dados, onde `VERDADEIRO`/`FALSO`
    /// no pool são a célula lógica (não casa com critério indexável); o
    /// resultado de fórmula guarda lógico à parte, então ali é texto mesmo.
    pub(crate) fn do_cru(&mut self, cru: Cru, literal: bool) -> Result<Option<u32>, ()> {
        let id = match cru {
            Cru::Numero(n) => return self.numero(n),
            Cru::Texto(id) => id,
        };
        let Ok(at) = usize::try_from(id) else { return Ok(None) };
        let ja = if literal { &self.por_id_literal } else { &self.por_id };
        match ja.get(at).copied() {
            Some(NAO_CASA) => return Ok(None),
            Some(c) if c != NAO_VISTO => return Ok(Some(c)),
            _ => {}
        }
        let texto = (self.texto_do_pool)(id);
        let codigo = if texto.is_empty() || (literal && (texto == "VERDADEIRO" || texto == "FALSO")) {
            NAO_CASA
        } else {
            self.codigo(ChaveCriterio::Texto(minusculas(texto)))
        };
        let tabela = if literal { &mut self.por_id_literal } else { &mut self.por_id };
        if at >= tabela.len() {
            tabela.resize(at + 1, NAO_VISTO);
        }
        tabela[at] = codigo;
        Ok((codigo != NAO_CASA).then_some(codigo))
    }

    /// O código de uma célula já calculada (a regra de
    /// [`ChaveCriterio::da_celula`]), sem passar o texto para minúscula de
    /// novo a cada linha.
    pub(crate) fn da_celula(&mut self, v: &Value) -> Result<Option<u32>, ()> {
        match v {
            Value::Number(n) => self.numero(*n),
            Value::Text(t) if !t.is_empty() => {
                if let Some(&c) = self.por_texto.get(t.as_str()) {
                    return Ok(Some(c));
                }
                let c = self.codigo(ChaveCriterio::Texto(minusculas(t)));
                self.por_texto.insert(t.as_str().into(), c);
                Ok(Some(c))
            }
            _ => Ok(None),
        }
    }
}

impl ChaveCriterio {
    /// A chave de um critério, quando ele é indexável: número finito e
    /// diferente de zero (zero também casa com célula vazia), ou texto não
    /// vazio sem operador (`>`, `<`, `=`) nem curinga (`*`, `?`).
    pub fn do_criterio(v: &Value) -> Option<Self> {
        match v {
            Value::Number(n) if n.is_finite() && *n != 0.0 => Some(ChaveCriterio::Numero(bits(*n))),
            Value::Text(t) => {
                let aparado = t.trim();
                if aparado.is_empty() || t.contains('*') || t.contains('?')
                    || aparado.starts_with(['<', '>', '='])
                    // `"2"` casa também o número 2: fica com a varredura.
                    || crate::celulas::number::from_js_string(&aparado.replacen(',', ".", 1)).is_some() {
                    return None;
                }
                Some(ChaveCriterio::Texto(minusculas(t)))
            }
            _ => None,
        }
    }

    /// A chave de um valor procurado por igualdade exata (`PROCX` modo 0):
    /// como [`Self::do_criterio`], mas sem a regra de operador e curinga —
    /// ali o texto é literal. Zero e vazio ficam de fora (casam entre si).
    pub fn exata(v: &Value) -> Option<Self> {
        match v {
            Value::Number(n) if n.is_finite() && *n != 0.0 => Some(ChaveCriterio::Numero(bits(*n))),
            Value::Text(t) if !t.is_empty() => Some(ChaveCriterio::Texto(minusculas(t))),
            _ => None,
        }
    }

    /// A chave de uma célula de critério. `Ok(None)`: a célula não casa com
    /// critério indexável nenhum (vazia, lógico, erro). `Err(())`: um `NaN`,
    /// que casa com qualquer número — o agregado não serve para esta faixa.
    pub(crate) fn da_celula(v: &Value) -> Result<Option<Self>, ()> {
        match v {
            Value::Number(n) if n.is_nan() => Err(()),
            Value::Number(n) => Ok(Some(ChaveCriterio::Numero(bits(*n)))),
            Value::Text(t) if !t.is_empty() => Ok(Some(ChaveCriterio::Texto(minusculas(t)))),
            _ => Ok(None),
        }
    }
}

/// O que se acumulou das linhas de um grupo.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Grupo {
    /// Linhas cujos critérios casaram (o `CONT.SES`).
    pub linhas: u32,
    /// Dessas, quantas têm número na faixa agregada (o `MÉDIASES` divide por isto).
    pub numeros: u32,
    pub soma: f64,
    pub maximo: f64,
    pub minimo: f64,
}

impl Grupo {
    fn acrescentar(&mut self, numero: Option<f64>) {
        self.linhas += 1;
        if let Some(n) = numero {
            if self.numeros == 0 {
                self.maximo = n;
                self.minimo = n;
            } else {
                self.maximo = self.maximo.max(n);
                self.minimo = self.minimo.min(n);
            }
            self.numeros += 1;
            self.soma += n;
        }
    }
}

/// Os grupos de um conjunto de faixas de critério (e de uma faixa agregada,
/// quando há).
#[derive(Debug, Default)]
pub struct Agregado {
    grupos: HashMap<Vec<ChaveCriterio>, Grupo>,
}

impl Agregado {
    /// O grupo da tupla de critérios; vazio (zeros) quando nenhuma linha casou.
    pub fn grupo(&self, chaves: &[ChaveCriterio]) -> Grupo {
        self.grupos.get(chaves).copied().unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.grupos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.grupos.is_empty()
    }

    /// Monta o agregado lendo cada posição `(linha, coluna)` de
    /// `0..linhas × 0..colunas`: `chave(i, r_off, c_off)` dá o código
    /// ([`LeitorChaves`]) da faixa de critério `i` (`Err`: um `NaN`, desiste)
    /// e `numero(r_off, c_off)` o número da faixa agregada, se for número.
    /// `chaves` traduz os códigos de volta no fim. `None` quando um `NaN`
    /// aparece ou há grupos demais.
    pub(crate) fn montar(
        n_criterios: usize,
        tem_soma: bool,
        linhas: u32,
        colunas: u32,
        mut chave: impl FnMut(usize, u32, u32) -> Result<Option<u32>, ()>,
        mut numero: impl FnMut(u32, u32) -> Option<f64>,
        chaves: impl FnOnce() -> Vec<ChaveCriterio>,
    ) -> Option<Self> {
        // Até 4 critérios a tupla de códigos cabe num `u128`; acima, `Vec`.
        let mut curtos: HashMap<u128, Grupo, std::hash::BuildHasherDefault<crate::celulas::mapa_chave::HasherChave>> = Default::default();
        let mut longos: HashMap<Vec<u32>, Grupo> = HashMap::new();
        let curto = n_criterios <= 4;
        let mut tupla: Vec<u32> = Vec::with_capacity(n_criterios);
        for r in 0..linhas {
            'posicao: for c in 0..colunas {
                tupla.clear();
                for i in 0..n_criterios {
                    match chave(i, r, c) {
                        Ok(Some(codigo)) => tupla.push(codigo),
                        Ok(None) => continue 'posicao,
                        Err(()) => return None,
                    }
                }
                let valor = if tem_soma { numero(r, c) } else { None };
                let grupo = if curto {
                    let k = tupla.iter().fold(0u128, |k, &c| (k << 32) | c as u128);
                    if curtos.len() >= MAX_GRUPOS && !curtos.contains_key(&k) {
                        return None;
                    }
                    curtos.entry(k).or_default()
                } else {
                    if longos.len() >= MAX_GRUPOS && !longos.contains_key(&tupla) {
                        return None;
                    }
                    longos.entry(tupla.clone()).or_default()
                };
                grupo.acrescentar(valor);
            }
        }
        let chaves = chaves();
        let traduzir = |codigos: &mut dyn Iterator<Item = u32>| -> Vec<ChaveCriterio> {
            codigos.map(|c| chaves[c as usize].clone()).collect()
        };
        let mut grupos = HashMap::with_capacity(curtos.len() + longos.len());
        for (k, g) in curtos {
            let mut codigos = (0..n_criterios).rev().map(|i| (k >> (32 * i)) as u32);
            grupos.insert(traduzir(&mut codigos), g);
        }
        for (k, g) in longos {
            grupos.insert(traduzir(&mut k.into_iter()), g);
        }
        Some(Agregado { grupos })
    }
}

/// Um retângulo sem o nome da aba, como chave.
pub(crate) type Area = (u32, u32, u32, u32);

/// Os agregados guardados de uma aba, válidos enquanto ela não muda.
#[derive(Default)]
pub(crate) struct Agregados {
    /// A geração da aba quando foram montados (ver `Sheet::geracao`).
    pub geracao: u64,
    /// `(faixa agregada, faixas de critério) → agregado`.
    pub mapa: HashMap<(Option<Area>, Vec<Area>), Arc<Agregado>>,
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn criterio_indexavel_e_o_de_igualdade_simples() {
        assert_eq!(ChaveCriterio::do_criterio(&Value::Number(2.0)), Some(ChaveCriterio::Numero(2.0f64.to_bits())));
        assert_eq!(ChaveCriterio::do_criterio(&Value::Number(0.0)), None);
        assert_eq!(ChaveCriterio::do_criterio(&Value::Number(f64::NAN)), None);
        assert_eq!(ChaveCriterio::do_criterio(&Value::Text("Sul".into())), Some(ChaveCriterio::Texto(Arc::from("sul"))));
        assert_eq!(ChaveCriterio::do_criterio(&Value::Text("ÁGUA".into())), Some(ChaveCriterio::Texto(Arc::from("água"))));
        assert_eq!(ChaveCriterio::do_criterio(&Value::Text(">10".into())), None);
        assert_eq!(ChaveCriterio::do_criterio(&Value::Text("a*".into())), None);
        assert_eq!(ChaveCriterio::do_criterio(&Value::Text("".into())), None);
        assert_eq!(ChaveCriterio::do_criterio(&Value::Bool(true)), None);
        assert_eq!(ChaveCriterio::do_criterio(&Value::Empty), None);
    }

    #[test]
    fn celula_casa_com_o_criterio_pela_mesma_regra() {
        assert_eq!(ChaveCriterio::da_celula(&Value::Number(-0.0)), Ok(Some(ChaveCriterio::Numero(0))));
        assert_eq!(ChaveCriterio::da_celula(&Value::Text("SUL".into())), Ok(Some(ChaveCriterio::Texto(Arc::from("sul")))));
        assert_eq!(ChaveCriterio::da_celula(&Value::Empty), Ok(None));
        assert_eq!(ChaveCriterio::da_celula(&Value::Bool(true)), Ok(None));
        assert_eq!(ChaveCriterio::da_celula(&Value::Number(f64::NAN)), Err(()));
    }

    #[test]
    fn agrupa_e_acumula() {
        // Critério 0: região; critério 1: ano; soma: valor.
        let regiao = ["Sul", "Norte", "sul", "Sul", "Norte", ""];
        let ano = [1.0, 1.0, 1.0, 2.0, 1.0, 1.0];
        let valor = [Value::Number(10.0), Value::Number(5.0), Value::Number(7.0), Value::Number(100.0), Value::Text("x".into()), Value::Number(1.0)];
        let sem_pool = |_: i32| "";
        let leitor = std::cell::RefCell::new(LeitorChaves::novo(&sem_pool));
        let ag = Agregado::montar(2, true, 6, 1,
            |i, r, _| leitor.borrow_mut().da_celula(&match i {
                0 => Value::Text(regiao[r as usize].into()),
                _ => Value::Number(ano[r as usize]),
            }),
            |r, _| match valor[r as usize] { Value::Number(n) => Some(n), _ => None },
            || std::mem::take(&mut leitor.borrow_mut().chaves),
        ).expect("monta");
        let sul1 = ag.grupo(&[ChaveCriterio::Texto(Arc::from("sul")), ChaveCriterio::Numero(1.0f64.to_bits())]);
        assert_eq!(sul1, Grupo { linhas: 2, numeros: 2, soma: 17.0, maximo: 10.0, minimo: 7.0 });
        let norte1 = ag.grupo(&[ChaveCriterio::Texto(Arc::from("norte")), ChaveCriterio::Numero(1.0f64.to_bits())]);
        assert_eq!(norte1, Grupo { linhas: 2, numeros: 1, soma: 5.0, maximo: 5.0, minimo: 5.0 });
        let nada = ag.grupo(&[ChaveCriterio::Texto(Arc::from("leste")), ChaveCriterio::Numero(1.0f64.to_bits())]);
        assert_eq!(nada, Grupo::default());
        // A linha com região vazia não entra em grupo nenhum.
        assert_eq!(ag.len(), 3);
    }

    #[test]
    fn nan_desiste_do_agregado() {
        let sem_pool = |_: i32| "";
        let mut leitor = LeitorChaves::novo(&sem_pool);
        let ag = Agregado::montar(1, false, 3, 1,
            |_, r, _| leitor.da_celula(&if r == 1 { Value::Number(f64::NAN) } else { Value::Number(1.0) }),
            |_, _| None, Vec::new);
        assert!(ag.is_none());
    }

    #[test]
    fn leitor_de_chaves_reaproveita_por_id_e_respeita_logico() {
        let textos = ["Sul", "VERDADEIRO", ""];
        let texto = |id: i32| textos[id as usize];
        let mut leitor = LeitorChaves::novo(&texto);
        let mut chave = |cru, literal| leitor.do_cru(cru, literal).map(|c| c.map(|c| c as usize));
        assert_eq!(chave(Cru::Texto(0), true), Ok(Some(0)));
        assert_eq!(chave(Cru::Texto(0), true), Ok(Some(0)), "mesmo código");
        assert_eq!(chave(Cru::Texto(1), true), Ok(None));
        assert_eq!(chave(Cru::Texto(1), false), Ok(Some(1)));
        assert_eq!(chave(Cru::Texto(2), false), Ok(None));
        assert_eq!(chave(Cru::Numero(-0.0), true), Ok(Some(2)));
        assert_eq!(chave(Cru::Numero(f64::NAN), true), Err(()));
        assert_eq!(leitor.chaves, vec![ChaveCriterio::Texto(Arc::from("sul")),
            ChaveCriterio::Texto(Arc::from("verdadeiro")), ChaveCriterio::Numero(0)]);
        // "SUL" escrito de outro jeito numa célula calculada: o mesmo código.
        assert_eq!(leitor.da_celula(&Value::Text("SUL".into())), Ok(Some(0)));
    }
}
