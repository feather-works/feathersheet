//! Caixas de seleção nas células (Inserir > Checkbox), como no Excel 365.
//!
//! Não há campo novo na aba: a caixa é a regra de validação `Caixa`
//! (`marcado`/`desmarcado`, por padrão VERDADEIRO/FALSO) que já existia em
//! Dados > Validação. A grade desenha a caixa em toda célula da regra cujo
//! valor é o marcado, o desmarcado ou vazio (vazio = desmarcada); qualquer
//! outro conteúdo aparece como texto. Fórmulas leem o valor normal.
use feathersheet_core::{
    celulas::{refs::Rect, values::Value},
    operacoes::validation::{QuandoValidacao, Regra, Validacao},
    planilha::sheet::Sheet,
};

pub const MARCADO: &str = "VERDADEIRO";
pub const DESMARCADO: &str = "FALSO";

/// As regras de validação da aba, lidas uma vez (por quadro ou por ação).
#[derive(Debug, Clone, Default)]
pub struct Regras {
    regras: Vec<Validacao>,
    tem_caixa: bool,
}

impl Regras {
    pub fn da(sheet: &Sheet) -> Self {
        // A maioria das abas não tem caixa: nem analisa o JSON.
        let Some(json) = sheet.va.as_deref().filter(|j| j.contains("\"caixa\"")) else {
            return Self::default();
        };
        let regras: Vec<Validacao> = serde_json::from_str(json).unwrap_or_default();
        let tem_caixa = regras.iter().any(|v| matches!(v.regra, Regra::Caixa { .. }));
        Self { regras, tem_caixa }
    }

    pub fn tem_caixa(&self) -> bool {
        self.tem_caixa
    }

    /// `(marcado, desmarcado)` quando a regra que vale na célula (a mais
    /// nova que a contém, como a validação) é uma caixa.
    pub fn caixa_em(&self, r: u32, c: u32) -> Option<(&str, &str)> {
        if !self.tem_caixa {
            return None;
        }
        let v = self.regras.iter().rev().find(|v| v.ranges.iter().any(|a| a.contains(r, c)))?;
        match &v.regra {
            Regra::Caixa { marcado, desmarcado } => Some((marcado, desmarcado)),
            _ => None,
        }
    }

    /// Alguma caixa encosta em `area`? (O botão Checkbox então remove.)
    pub fn alguma_caixa_em(&self, area: &Rect) -> bool {
        self.tem_caixa && self.regras.iter()
            .filter(|v| matches!(v.regra, Regra::Caixa { .. }))
            .flat_map(|v| &v.ranges)
            .any(|r| r.r0 <= area.r1 && area.r0 <= r.r1 && r.c0 <= area.c1 && area.c0 <= r.c1)
    }

    /// O JSON com uma caixa VERDADEIRO/FALSO nova em `area` (por cima das
    /// regras que já valem ali, como a validação mais nova).
    pub fn com_caixa(&self, sheet: &Sheet, area: &Rect) -> Option<String> {
        let mut regras: Vec<Validacao> = sheet.va.as_deref()
            .and_then(|j| serde_json::from_str(j).ok()).unwrap_or_default();
        regras.push(Validacao {
            ranges: vec![area.clone()],
            regra: Regra::Caixa { marcado: MARCADO.into(), desmarcado: DESMARCADO.into() },
            quando: QuandoValidacao::Avisar,
            mensagem: None,
            mensagens: Default::default(),
        });
        serde_json::to_string(&regras).ok()
    }

    /// O JSON sem as caixas dentro de `area` (Limpar formatação), ou `None`
    /// se nada muda. Os intervalos que só encostam em parte são recortados.
    pub fn sem_caixas_em(&self, area: &Rect) -> Option<String> {
        self.sem_caixas_nas(std::slice::from_ref(area))
    }

    /// [`Self::sem_caixas_em`] para várias áreas (seleção com Ctrl).
    pub fn sem_caixas_nas(&self, areas: &[Rect]) -> Option<String> {
        if !self.tem_caixa {
            return None;
        }
        let mut mudou = false;
        let mut regras = self.regras.clone();
        for v in regras.iter_mut().filter(|v| matches!(v.regra, Regra::Caixa { .. })) {
            let antes = v.ranges.len();
            let mut novos = v.ranges.clone();
            for area in areas {
                novos = novos.iter().flat_map(|r| subtrair(r, area)).collect();
            }
            if novos.len() != antes || novos.iter().zip(&v.ranges).any(|(a, b)| a != b) {
                mudou = true;
            }
            v.ranges = novos;
        }
        regras.retain(|v| !v.ranges.is_empty());
        mudou.then(|| serde_json::to_string(&regras).ok()).flatten()
    }
}

/// O valor da célula como a validação compara (VERDADEIRO/FALSO, texto…).
fn texto_valor(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Text(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => if *b { MARCADO.into() } else { DESMARCADO.into() },
        Value::Error(e) => e.to_string(),
    }
}

/// `Some(true)` marcada, `Some(false)` desmarcada (ou vazia), `None` = a
/// célula tem outro conteúdo e não aparece como caixa.
pub fn estado(sheet: &Sheet, r: u32, c: u32, marcado: &str, desmarcado: &str) -> Option<bool> {
    let v = sheet.get_value(r, c);
    if matches!(v, Value::Empty) {
        return Some(false);
    }
    let t = texto_valor(&v);
    if t.eq_ignore_ascii_case(marcado) {
        Some(true)
    } else if t.eq_ignore_ascii_case(desmarcado) {
        Some(false)
    } else {
        None
    }
}

/// `a` menos `b`: até quatro retângulos (faixas de cima, de baixo e laterais).
fn subtrair(a: &Rect, b: &Rect) -> Vec<Rect> {
    let (r0, r1) = (a.r0.max(b.r0), a.r1.min(b.r1));
    let (c0, c1) = (a.c0.max(b.c0), a.c1.min(b.c1));
    if r0 > r1 || c0 > c1 {
        return vec![a.clone()];
    }
    let mut v = Vec::new();
    if a.r0 < r0 { v.push(Rect::new(a.r0, a.c0, r0 - 1, a.c1)); }
    if r1 < a.r1 { v.push(Rect::new(r1 + 1, a.c0, a.r1, a.c1)); }
    if a.c0 < c0 { v.push(Rect::new(r0, a.c0, r1, c0 - 1)); }
    if c1 < a.c1 { v.push(Rect::new(r0, c1 + 1, r1, a.c1)); }
    v
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn subtrair_recorta_em_volta_e_some_quando_cobre_tudo() {
        let a = Rect::new(0, 0, 9, 9);
        assert_eq!(subtrair(&a, &Rect::new(0, 0, 9, 9)), vec![]);
        assert_eq!(subtrair(&a, &Rect::new(20, 20, 30, 30)), vec![a.clone()]);
        let partes = subtrair(&a, &Rect::new(3, 3, 5, 5));
        assert_eq!(partes.len(), 4);
        let celulas: u32 = partes.iter().map(|r| (r.r1 - r.r0 + 1) * (r.c1 - r.c0 + 1)).sum();
        assert_eq!(celulas, 100 - 9);
    }
}
