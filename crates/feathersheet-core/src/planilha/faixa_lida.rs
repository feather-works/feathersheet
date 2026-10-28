//! Faixa lida de uma vez, para as funções de critério (`SOMASES`,
//! `CONT.SES`, `MÉDIASES`…) não perguntarem célula por célula.
//!
//! Antes, cada `SOMASES` de 1 milhão de linhas fazia 3 milhões de perguntas
//! `get_value` — cada uma resolvendo o nome da aba, procurando em mapas e
//! clonando o valor —, e a aba Resumo, com ~150 delas, levava 84 s (release)
//! travando a janela. Agora cada coluna é lida uma vez e fica guardada até a
//! aba mudar ([`Faixas::geracao`]).

use std::collections::HashMap;
use std::sync::Arc;

use crate::celulas::values::Value;

/// Abaixo disto ler célula por célula já é barato. Pequeno de propósito:
/// a lista de 200 produtos de um `PROCX` é lida por um milhão de linhas.
pub(crate) const MINIMO: u64 = 128;
/// Uma faixa maior que isto não é guardada (memória).
pub(crate) const MAXIMO: u64 = 4_000_000;
/// Soma das células guardadas numa aba; passou, esvazia tudo.
pub(crate) const TOTAL: usize = 12_000_000;

/// Os valores de um retângulo, linha a linha. Fora do retângulo guardado
/// (além da última linha/coluna usada da aba) a célula é vazia.
#[derive(Debug)]
pub struct FaixaLida {
    r0: u32,
    c0: u32,
    linhas: u32,
    colunas: u32,
    valores: Vec<Value>,
    /// Posição do primeiro valor igual a cada chave, para a busca exata
    /// (`PROCX` modo 0) não varrer a lista a cada uma do milhão de linhas
    /// que a consultam. Montado na primeira busca; `None` quando a faixa não
    /// é um vetor ou tem `NaN` (que casa com qualquer número).
    indice_exato: std::sync::OnceLock<Option<HashMap<crate::planilha::agregado::ChaveCriterio, u32>>>,
}

static VAZIO: Value = Value::Empty;

impl FaixaLida {
    pub(crate) fn nova(r0: u32, c0: u32, linhas: u32, colunas: u32, valores: Vec<Value>) -> Self {
        debug_assert_eq!(valores.len() as u64, linhas as u64 * colunas as u64);
        FaixaLida { r0, c0, linhas, colunas, valores, indice_exato: std::sync::OnceLock::new() }
    }

    /// A posição (no vetor: linha ou coluna, a partir de zero) do primeiro
    /// valor igual a `procurado`, pela regra de
    /// [`crate::funcoes::fn_helpers::valores_iguais`]. `None` de fora: a
    /// busca não é indexável (procurado vazio, zero, lógico; faixa com
    /// `NaN`), varrer como sempre. `Some(None)`: não há.
    pub fn posicao_exata(&self, procurado: &Value) -> Option<Option<u32>> {
        use crate::planilha::agregado::ChaveCriterio;
        let chave = ChaveCriterio::exata(procurado)?;
        let indice = self.indice_exato.get_or_init(|| {
            if self.linhas != 1 && self.colunas != 1 {
                return None;
            }
            let mut mapa = HashMap::new();
            for (i, v) in self.valores.iter().enumerate() {
                match ChaveCriterio::da_celula(v) {
                    Ok(Some(c)) => { mapa.entry(c).or_insert(i as u32); }
                    Ok(None) => {}
                    Err(()) => return None,
                }
            }
            Some(mapa)
        }).as_ref()?;
        Some(indice.get(&chave).copied())
    }

    /// Cada valor não vazio, com a linha e a coluna dele na aba.
    pub fn para_cada(&self, visit: &mut dyn FnMut(&Value, u32, u32)) {
        if self.colunas == 0 {
            return;
        }
        for (i, v) in self.valores.iter().enumerate() {
            if !matches!(v, Value::Empty) {
                let (r, c) = (i / self.colunas as usize, i % self.colunas as usize);
                visit(v, self.r0 + r as u32, self.c0 + c as u32);
            }
        }
    }

    /// O valor da célula (`row`, `col`) da aba — não da faixa.
    pub fn valor(&self, row: u32, col: u32) -> &Value {
        let (Some(r), Some(c)) = (row.checked_sub(self.r0), col.checked_sub(self.c0)) else {
            return &VAZIO;
        };
        if r >= self.linhas || c >= self.colunas {
            return &VAZIO;
        }
        &self.valores[r as usize * self.colunas as usize + c as usize]
    }
}

/// As faixas guardadas de uma aba.
#[derive(Default)]
pub(crate) struct Faixas {
    /// A geração da aba quando estas faixas foram lidas.
    pub geracao: u64,
    pub celulas: usize,
    pub mapa: HashMap<(u32, u32, u32, u32), Arc<FaixaLida>>,
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::celulas::values::{CellError, ErrorCode};
    use crate::funcoes::fn_helpers::valores_iguais;

    /// O índice responde o mesmo que a varredura do `PROCX` exato (o
    /// primeiro igual pela regra de `valores_iguais`), ou se recusa.
    #[test]
    fn indice_exato_igual_a_varredura() {
        let t = |s: &str| Value::Text(s.into());
        let n = Value::Number;
        let lista = vec![
            t("P1"), t("p1"), n(10.0), t("10"), Value::Empty, Value::Error(CellError::new(ErrorCode::Na)),
            Value::Bool(true), t("ÁGUA"), t("água"), n(10.0), n(0.30000000000000004), n(-0.0), t(""),
            n(1.0), t("verdadeiro"), n(f64::INFINITY),
        ];
        let procurados = vec![
            t("p1"), t("P1"), n(10.0), t("10"), t("Água"), n(0.3), n(0.30000000000000004), n(1.0),
            Value::Bool(true), t("VERDADEIRO"), t("nada"), n(99.0), n(f64::INFINITY),
            // Fora do índice (zero e vazio casam entre si; NaN com qualquer número).
            n(0.0), n(-0.0), t(""), Value::Empty, n(f64::NAN),
        ];
        for (linhas, colunas) in [(lista.len() as u32, 1), (1, lista.len() as u32)] {
            let faixa = FaixaLida::nova(0, 0, linhas, colunas, lista.clone());
            for p in &procurados {
                let varrido = lista.iter().position(|v| valores_iguais(v, p)).map(|i| i as u32);
                match faixa.posicao_exata(p) {
                    Some(achado) => assert_eq!(achado, varrido, "procurando {p:?}"),
                    None => assert!(matches!(p, Value::Empty | Value::Bool(_))
                        || matches!(p, Value::Number(x) if *x == 0.0 || !x.is_finite())
                        || matches!(p, Value::Text(s) if s.is_empty()), "recusou {p:?}"),
                }
            }
        }
        // Lista com NaN: casa com qualquer número, o índice se recusa.
        let com_nan = FaixaLida::nova(0, 0, 2, 1, vec![n(f64::NAN), n(1.0)]);
        assert_eq!(com_nan.posicao_exata(&n(1.0)), None);
        // Área de duas dimensões: sem índice.
        let area = FaixaLida::nova(0, 0, 2, 2, vec![n(1.0), n(2.0), n(3.0), n(4.0)]);
        assert_eq!(area.posicao_exata(&n(2.0)), None);
    }
}
