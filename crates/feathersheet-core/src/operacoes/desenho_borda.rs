//! Desenhar bordas (Página Inicial > Fonte > Bordas ▾ > Desenhar borda).
//!
//! Como no Excel: com a caneta de borda, arrastar sobre as células desenha o
//! contorno da área ("Desenhar borda"), todas as linhas internas e externas
//! ("Desenhar grade de borda") ou apaga as bordas ("Apagar borda"). Cada
//! traço é um Ctrl+Z. A cor e o estilo da linha (`borderLineStyle`: Fina,
//! Média, Grossa, Tracejada, o mesmo campo do Formatar células) vão junto.
use std::collections::BTreeMap;

use crate::celulas::estilo_parcial::{Campo, EstiloParcial};
use crate::celulas::refs::Rect;
use crate::planilha::sheet::Sheet;

/// Teto de células de um traço: arrastar sobre a planilha inteira não pode
/// virar milhões de estilos de célula.
pub const LIMITE_CELULAS: u64 = 250_000;

/// Os lados de uma célula, na grafia do estilo.
const LADOS: [&str; 4] = ["top", "right", "bottom", "left"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoBorda {
    /// Só o contorno da área arrastada.
    Contorno,
    /// Contorno e todas as linhas internas.
    Grade,
    /// Tira as bordas das células (e o lado vizinho que as toca).
    Apagar,
}

/// A caneta de borda: cor (`None` = automática) e estilo da linha
/// (`None` = Fina, o padrão).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanetaBorda {
    pub cor: Option<String>,
    pub estilo: Option<String>,
}

/// Estilos de linha aceitos (os mesmos do Formatar células > Borda).
pub const ESTILOS: [&str; 4] = ["Fina", "Média", "Grossa", "Tracejada"];

fn bordas_da_celula(sheet: &Sheet, linha: u32, coluna: u32) -> BTreeMap<String, Campo> {
    match sheet.estilo_mesclado(linha, coluna).and_then(|mut e| e.remove("borders")) {
        Some(Campo::Objeto(b)) => b,
        _ => BTreeMap::new(),
    }
}

fn ligado(b: &BTreeMap<String, Campo>, lado: &str) -> bool {
    matches!(b.get(lado), Some(Campo::Logico(true)))
}

/// Grava as bordas da célula; sem lado nenhum, tira o campo (e o estilo da linha).
fn gravar(sheet: &mut Sheet, linha: u32, coluna: u32, b: BTreeMap<String, Campo>, estilo: Option<Campo>) -> bool {
    let antes = sheet.estilo_mesclado(linha, coluna);
    let mut patch = EstiloParcial::new();
    if LADOS.iter().any(|l| ligado(&b, l)) {
        patch.insert("borders".into(), Campo::Objeto(b));
        if let Some(e) = estilo { patch.insert("borderLineStyle".into(), e); }
    } else {
        patch.insert("borders".into(), Campo::Nulo);
        patch.insert("borderLineStyle".into(), Campo::Nulo);
    }
    let depois = crate::celulas::estilo_parcial::mesclar(antes.as_ref(), &patch);
    if antes.unwrap_or_default() == depois { return false; }
    sheet.set_style(&Rect::cell(linha, coluna), &patch);
    true
}

/// Aplica um traço da caneta de borda na `area`, numa só transação.
/// Devolve se algo mudou, ou o motivo de recusar.
pub fn desenhar_borda(sheet: &mut Sheet, area: &Rect, modo: ModoBorda, caneta: &CanetaBorda) -> Result<bool, String> {
    let r = area.normalized();
    let total = (r.r1 - r.r0 + 1) as u64 * (r.c1 - r.c0 + 1) as u64;
    if total > LIMITE_CELULAS {
        return Err(crate::trf!("Área grande demais para desenhar bordas ({total} células; o limite é {LIMITE_CELULAS})."));
    }
    let estilo = match caneta.estilo.as_deref() {
        None | Some("Fina") => Campo::Nulo,
        Some(e) if ESTILOS.contains(&e) => Campo::Texto(e.to_string()),
        Some(e) => return Err(crate::trf!("Estilo de linha desconhecido: {e}.")),
    };
    let mut mudou = false;
    sheet.abrir_transacao();
    for l in r.r0..=r.r1 {
        for c in r.c0..=r.c1 {
            let mut b = bordas_da_celula(sheet, l, c);
            match modo {
                ModoBorda::Apagar => b.clear(),
                ModoBorda::Contorno | ModoBorda::Grade => {
                    let grade = modo == ModoBorda::Grade;
                    let lados = [
                        grade || l == r.r0, grade || c == r.c1,
                        grade || l == r.r1, grade || c == r.c0,
                    ];
                    let mut tocou = false;
                    for (lado, liga) in LADOS.iter().zip(lados) {
                        if liga { b.insert((*lado).into(), Campo::Logico(true)); tocou = true; }
                    }
                    if !tocou { continue; }
                    match &caneta.cor {
                        Some(cor) => { b.insert("color".into(), Campo::Texto(cor.clone())); }
                        None => { b.remove("color"); }
                    }
                }
            }
            mudou |= gravar(sheet, l, c, b, Some(estilo.clone()));
        }
    }
    if modo == ModoBorda::Apagar {
        // A linha que se vê entre duas células pode ser da vizinha: apagar
        // também o lado dela que encosta na área.
        let mut vizinhas: Vec<(u32, u32, &str)> = Vec::new();
        if r.r0 > 0 { for c in r.c0..=r.c1 { vizinhas.push((r.r0 - 1, c, "bottom")); } }
        for c in r.c0..=r.c1 { vizinhas.push((r.r1 + 1, c, "top")); }
        if r.c0 > 0 { for l in r.r0..=r.r1 { vizinhas.push((l, r.c0 - 1, "right")); } }
        for l in r.r0..=r.r1 { vizinhas.push((l, r.c1 + 1, "left")); }
        for (l, c, lado) in vizinhas {
            let mut b = bordas_da_celula(sheet, l, c);
            if b.remove(lado).is_some() {
                mudou |= gravar(sheet, l, c, b, None);
            }
        }
    }
    sheet.fechar_transacao();
    Ok(mudou)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn lados(s: &Sheet, l: u32, c: u32) -> String {
        let b = bordas_da_celula(s, l, c);
        LADOS.iter().map(|lado| if ligado(&b, lado) { lado.chars().next().unwrap() } else { '-' }).collect()
    }

    #[test]
    fn contorno_grade_e_apagar_num_ctrl_z_cada() {
        let mut s = Sheet::new("T");
        let caneta = CanetaBorda { cor: Some("#d93025".into()), estilo: Some("Grossa".into()) };
        assert!(desenhar_borda(&mut s, &Rect::new(1, 1, 3, 3), ModoBorda::Contorno, &caneta).unwrap());
        assert_eq!(lados(&s, 1, 1), "t--l");
        assert_eq!(lados(&s, 2, 2), "----");
        assert_eq!(lados(&s, 3, 3), "-rb-");
        let e = s.estilo_mesclado(1, 1).unwrap();
        assert_eq!(e.get("borderLineStyle"), Some(&Campo::Texto("Grossa".into())));
        assert!(matches!(e.get("borders"), Some(Campo::Objeto(b)) if b.get("color") == Some(&Campo::Texto("#d93025".into()))));

        assert!(desenhar_borda(&mut s, &Rect::new(2, 2, 2, 2), ModoBorda::Grade, &CanetaBorda::default()).unwrap());
        assert_eq!(lados(&s, 2, 2), "trbl");
        assert!(!s.estilo_mesclado(2, 2).unwrap().contains_key("borderLineStyle"));

        // Apagar a célula do meio também tira o lado das vizinhas que a toca.
        assert!(desenhar_borda(&mut s, &Rect::new(1, 1, 1, 1), ModoBorda::Apagar, &caneta).unwrap());
        assert_eq!(lados(&s, 1, 1), "----");
        assert!(s.estilo_mesclado(1, 1).map_or(true, |e| !e.contains_key("borders")));

        assert!(s.undo());
        assert_eq!(lados(&s, 1, 1), "t--l");
        assert!(s.undo());
        assert_eq!(lados(&s, 2, 2), "----");
        assert!(s.undo());
        assert_eq!(lados(&s, 1, 1), "----");
    }

    #[test]
    fn apagar_tira_o_lado_da_vizinha_e_area_enorme_e_recusada() {
        let mut s = Sheet::new("T");
        desenhar_borda(&mut s, &Rect::new(0, 0, 0, 0), ModoBorda::Grade, &CanetaBorda::default()).unwrap();
        // A célula de baixo não tem borda, mas a linha entre as duas é o "bottom" de A1.
        assert!(desenhar_borda(&mut s, &Rect::new(1, 0, 1, 0), ModoBorda::Apagar, &CanetaBorda::default()).unwrap());
        assert_eq!(lados(&s, 0, 0), "tr-l");
        assert!(desenhar_borda(&mut s, &Rect::new(0, 0, 999_999, 5), ModoBorda::Grade, &CanetaBorda::default()).is_err());
        let ruim = CanetaBorda { cor: None, estilo: Some("Pontilhada".into()) };
        assert!(desenhar_borda(&mut s, &Rect::cell(0, 0), ModoBorda::Grade, &ruim).is_err());
    }
}
