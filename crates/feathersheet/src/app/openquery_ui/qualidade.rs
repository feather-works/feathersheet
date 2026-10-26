//! Prévia de qualidade e inferência de tipos portada de preview-grid.ts.
//! O limite de amostragem impede varrer a planilha inteira a cada redraw Iced.
use super::*;

const MAX_AMOSTRA: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tipo { Inteiro, Decimal, Logico, Data, Texto }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Estatistica {
    pub tipo: Tipo,
    pub analisadas: usize,
    pub validas: usize,
    pub vazias: usize,
    pub erros: usize,
}
impl Estatistica {
    pub(super) fn codigo(self) -> &'static str {
        match self.tipo {
            Tipo::Inteiro => "123", Tipo::Decimal => "1.2",
            Tipo::Logico => "T/F", Tipo::Data => "DATA", Tipo::Texto => "ABC",
        }
    }
    pub(super) fn resumo(self) -> String {
        if self.analisadas == 0 { return tr!("Sem amostra").into(); }
        format!(
            "✓{}%  ø{}%  !{}%",
            self.validas * 100 / self.analisadas,
            self.vazias * 100 / self.analisadas,
            self.erros * 100 / self.analisadas,
        )
    }
}

/// Contagens das 500 primeiras linhas, compatíveis com a heurística visual TS.
/// Não altera o tipo real armazenado no motor Rust.
pub(super) fn analisar(t: &Tabela, coluna: usize) -> Estatistica {
    let mut out = Estatistica {
        tipo: Tipo::Texto, analisadas: 0, validas: 0, vazias: 0, erros: 0,
    };
    let (mut numeros, mut inteiros, mut logicos, mut datas) = (0usize, 0usize, 0usize, 0usize);
    for linha in t.linhas.iter().take(MAX_AMOSTRA) {
        out.analisadas += 1;
        match linha.get(coluna).unwrap_or(&Value::Empty) {
            Value::Empty => out.vazias += 1,
            Value::Error(_) => out.erros += 1,
            Value::Bool(_) => { out.validas += 1; logicos += 1; }
            Value::Number(num) => {
                out.validas += 1;
                if num.is_finite() {
                    numeros += 1;
                    if num.fract() == 0.0 { inteiros += 1; }
                }
            }
            Value::Text(s) if s.trim().is_empty() => out.vazias += 1,
            Value::Text(s) => {
                out.validas += 1;
                if s == "true" || s == "false" { logicos += 1; }
                else if let Ok(num) = s.parse::<f64>() {
                    if num.is_finite() {
                        numeros += 1;
                        if num.fract() == 0.0 { inteiros += 1; }
                    }
                } else if s.contains('-') || s.contains('/') {
                    // Heurística visual, não converte strings em datas.
                    let separador = if s.contains('-') { '-' } else { '/' };
                    let partes = s.split(separador).collect::<Vec<_>>();
                    if partes.len() == 3 && partes.iter().all(|p| p.chars().all(|c|c.is_ascii_digit())) {
                        datas += 1;
                    }
                }
            }
        }
    }
    let total = out.validas;
    if total > 0 {
        out.tipo = if datas * 10 > total * 8 { Tipo::Data }
        else if logicos * 10 > total * 8 { Tipo::Logico }
        else if numeros * 10 > total * 8 {
            if inteiros * 10 > numeros * 8 { Tipo::Inteiro }
            else { Tipo::Decimal }
        } else { Tipo::Texto };
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn qualidade_separa_vazio_erro_e_texto_com_amostragem_limitada() {
        let t=Tabela {
            cabecalho:vec!["MWh".into()],
            linhas:vec![
                vec![Value::Number(3.)], vec![Value::Number(4.)],
                vec![Value::Empty], vec![Value::Text("".into())],
                vec![Value::Number(5.)],
            ],
        };
        let q=analisar(&t,0);
        assert_eq!(q.tipo,Tipo::Inteiro);
        assert_eq!((q.analisadas,q.validas,q.vazias,q.erros),(5,3,2,0));
        assert_eq!(q.resumo(),"✓60%  ø40%  !0%");
    }
    #[test]
    fn nao_percorre_milhao_de_linhas_para_renderizar() {
        let tabela = Tabela {
            cabecalho:vec!["C".into()],
            linhas:(0..5_000).map(|_|vec![Value::Bool(true)]).collect(),
        };
        let q = analisar(&tabela,0);
        assert_eq!(q.analisadas,500);
        assert_eq!(q.tipo,Tipo::Logico);
    }
}
