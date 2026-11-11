//! Mesclagem de células no formato do arquivo: o metadado `sheet.mc` é uma
//! lista JSON de retângulos. A geometria mora no motor
//! (`operacoes::mesclagem`); aqui fica o JSON, que o motor não lê. Usado pela
//! interface e pelo MCP, sem duplicar valores nem tocar nas células.
use feathersheet_core::{
    celulas::refs::Rect,
    operacoes::mesclagem as geo,
    planilha::{sheet::Sheet, workbook::Workbook},
};

pub fn cruzam(a: &Rect, b: &Rect) -> bool { geo::cruzam(a,b) }

/// As regiões mescladas da aba (JSON inválido não apaga as válidas).
pub fn ler(sheet: &Sheet) -> Vec<Rect> {
    sheet.mc.as_deref().map(ler_json).unwrap_or_default()
}

/// Usado pelo botão único de Mesclar: só alterna para Desmesclar quando a
/// seleção corresponde a uma região mesclada inteira.
pub fn exatamente_mesclada(sheet: &Sheet, area: &Rect) -> bool {
    ler(sheet).iter().any(|m| m == area)
}

pub fn em(regioes: &[Rect], r: u32, c: u32) -> Option<&Rect> {
    geo::em(regioes, r, c)
}

/// Selecionar parte de uma mescla seleciona o retângulo completo (como no TS).
pub fn expandir(regioes: &[Rect], area: Rect) -> Rect {
    geo::expandir(regioes, area)
}

/// Mesclar só modifica as regiões: outros valores continuam intactos
/// e voltam a aparecer quando se desmescla.
pub fn mesclar(sheet: &mut Sheet, area: Rect) -> bool {
    let lista = ler(sheet);
    // Invalidar toda a área expandida, não somente a seleção original.
    // Seleção sobreposta pode crescer ao incorporar mesclas vizinhas.
    let area = geo::expandir(&lista, area.normalized());
    let Some(novas) = geo::mesclar(&lista, area.clone()) else { return false; };
    let novo = serde_json::to_string(&novas).ok();
    sheet.alterar_mesclas(novo, &area)
}

pub fn desmesclar(sheet: &mut Sheet, area: Rect) -> bool {
    let lista = ler(sheet);
    let Some(novas) = geo::desmesclar(&lista, area.clone()) else { return false; };
    let novo = if novas.is_empty() { None } else { serde_json::to_string(&novas).ok() };
    sheet.alterar_mesclas(novo, &area)
}

/// Atualiza metadados de mesclagem após inserir ou excluir linhas/colunas.
/// Os dados já foram deslocados pelo Workbook; aqui movemos só os retângulos,
/// como `src/core/mesclagem.ts`. Nunca copiamos conteúdos nem recriamos
/// mesclas de uma única célula. Sem histórico individual: a mutação estrutural
/// do Workbook já não é desfeita via Ctrl+Z.
pub fn refluir(
    workbook: &mut Workbook, aba: usize, coluna: bool, at: u32,
    count: u32, removendo: bool,
) -> Result<(), String> {
    if count == 0 { return Ok(()); }
    let anterior = ler(&workbook.aba(aba)
        .ok_or_else(|| feathersheet_core::tr!("Aba da mesclagem indisponível"))?.sheet);
    if anterior.is_empty() { return Ok(()); }
    let delta = if removendo { -(count as i64) } else { count as i64 };
    let atualizada = feathersheet_core::operacoes::reflow::refluir_mesclas(
        &anterior, coluna, at, delta,
    );
    if anterior == atualizada { return Ok(()); }
    let novo = if atualizada.is_empty() { None } else {
        Some(serde_json::to_string(&atualizada)
            .map_err(|e| feathersheet_core::trf!("Falha ao serializar mesclagens: {e}"))?)
    };
    let viva = workbook.aba_mut(aba).ok_or_else(|| feathersheet_core::tr!("Aba da mesclagem indisponível"))?;
    viva.sheet.mc = novo;
    viva.versao = viva.versao.wrapping_add(1).max(1);
    Ok(())
}

fn numero_inteiro(v: &serde_json::Value) -> Option<u32> {
    if let Some(n) = v.as_u64() { return u32::try_from(n).ok(); }
    let n = v.as_f64()?;
    (n.is_finite() && n.fract() == 0.0 && n >= 0.0 && n <= u32::MAX as f64)
        .then_some(n as u32)
}

/// Lê o JSON legado com tolerância: entrada inválida é ignorada, as válidas ficam.
pub fn ler_json(json: &str) -> Vec<Rect> {
    let Ok(dado) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new(); };
    let Some(registros) = dado.as_array() else { return Vec::new(); };
    let mut regioes = Vec::with_capacity(registros.len().min(1024));
    for registro in registros {
        let campo = |nome: &str| registro.get(nome).and_then(numero_inteiro);
        let (Some(r0), Some(c0), Some(r1), Some(c1)) =
            (campo("r0"), campo("c0"), campo("r1"), campo("c1"))
        else { continue; };
        regioes.push(Rect::new(r0, c0, r1, c1));
    }
    // Centraliza normalização, limites, mesclas unitárias e sobreposições no core.
    geo::normalizar(regioes)
}

#[cfg(test)]
mod testes_json {
    use super::*;

    #[test]
    fn json_invalido_nao_apaga_mesclas_validas() {
        let json = r#"[{"r0":1,"c0":0,"r1":1,"c1":3},null,
          {"r0":"erro","c0":1,"r1":2,"c1":2},
          {"r0":4,"c0":2,"r1":4,"c1":5}]"#;
        assert_eq!(ler_json(json), vec![Rect::new(1,0,1,3), Rect::new(4,2,4,5)]);
    }

    #[test]
    fn inverte_extremos_e_ultima_mescla_sobreposta_prevalece() {
        let json = r#"[{"r0":5,"c0":5,"r1":3,"c1":3},
          {"r0":4.0,"c0":4,"r1":4,"c1":6}]"#;
        assert_eq!(ler_json(json), vec![Rect::new(4,4,4,6)]);
    }

    #[test]
    fn recusa_unitarias_invalidas_e_limites() {
        let json = r#"[{"r0":0,"c0":0,"r1":0,"c1":0},
          {"r0":true,"c0":0,"r1":0,"c1":3},
          {"r0":-1,"c0":0,"r1":0,"c1":3},
          {"r0":0,"c0":16384,"r1":0,"c1":16385},
          {"r0":0,"c0":2,"r1":0,"c1":3}]"#;
        assert_eq!(ler_json(json), vec![Rect::new(0,2,0,3)]);
        assert!(ler_json("{erro").is_empty());
        assert!(ler_json("{}").is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mesclar_desmesclar_preserva_conteudo_e_historico() {
        let mut sheet = Sheet::new("Plan1");
        sheet.set_cell(0, 0, "Título");
        sheet.set_cell(0, 1, "Segundo");
        let area = Rect::new(0, 0, 0, 3);
        assert!(mesclar(&mut sheet, area.clone()));
        assert_eq!(ler(&sheet), vec![area.clone()]);
        assert_eq!(em(&ler(&sheet), 0, 2), Some(&area));
        assert!(!mesclar(&mut sheet, area.clone()));
        assert!(desmesclar(&mut sheet, Rect::cell(0, 1)));
        assert!(ler(&sheet).is_empty());
        assert_eq!(sheet.get_raw(0, 1), "Segundo");
        assert!(sheet.undo());
        assert_eq!(ler(&sheet), vec![area.clone()]);
        assert!(sheet.undo());
        assert!(ler(&sheet).is_empty());
        assert!(sheet.redo());
        assert_eq!(ler(&sheet), vec![area]);
    }
    #[test]
    fn estado_do_botao_mesclar_somente_para_regiao_exata() {
        let mut sheet = Sheet::new("Plan1");
        let area = Rect::new(2, 1, 2, 4);
        assert!(!exatamente_mesclada(&sheet, &area));
        assert!(mesclar(&mut sheet, area.clone()));
        assert!(exatamente_mesclada(&sheet, &area));
        assert!(!exatamente_mesclada(&sheet, &Rect::new(2, 1, 3, 4)));
        assert!(desmesclar(&mut sheet, area.clone()));
        assert!(!exatamente_mesclada(&sheet, &area));
    }

    #[test]
    fn cruzamento_expandido_remove_mesclas_anteriores() {
        let mut sheet = Sheet::new("Plan1");
        assert!(mesclar(&mut sheet, Rect::new(0, 0, 1, 1)));
        assert!(mesclar(&mut sheet, Rect::new(1, 1, 2, 3)));
        assert_eq!(ler(&sheet), vec![Rect::new(0, 0, 2, 3)]);
    }
    #[test]
    fn inserir_excluir_reflui_mesclas_sem_duplicar_textos() {
        let mut pasta = Workbook::new();
        {
            let sheet = &mut pasta.aba_mut(0).unwrap().sheet;
            sheet.set_cell(1, 1, "Título");
            sheet.set_cell(1, 2, "Texto à direita");
            assert!(mesclar(sheet, Rect::new(1, 1, 1, 3)));
        }
        assert!(pasta.inserir_linhas(0, 0, 2).is_some());
        refluir(&mut pasta, 0, false, 0, 2, false).unwrap();
        assert_eq!(ler(&pasta.aba(0).unwrap().sheet), vec![Rect::new(3, 1, 3, 3)]);
        assert_eq!(pasta.valor(0, 3, 1), feathersheet_core::Value::Text("Título".into()));
        assert_eq!(pasta.valor(0, 3, 2), feathersheet_core::Value::Text("Texto à direita".into()));
        assert!(pasta.excluir_colunas(0, 2, 2).is_some());
        refluir(&mut pasta, 0, true, 2, 2, true).unwrap();
        assert!(ler(&pasta.aba(0).unwrap().sheet).is_empty());
        assert_eq!(pasta.valor(0, 3, 1), feathersheet_core::Value::Text("Título".into()));
    }

    #[test]
    fn formato_herdado_normaliza_mesclas_invalidas() {
        let mut sheet = Sheet::new("Plan1");
        sheet.mc = Some(r#"[{"r0":1,"c0":1,"r1":1,"c1":3},{"r0":7,"c0":7,"r1":7,"c1":7}]"#.into());
        assert_eq!(ler(&sheet), vec![Rect::new(1, 1, 1, 3)]);
    }
}
