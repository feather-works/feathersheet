//! Converte a `Workbook` nativa em Rust em `PastaBruta` e grava o `.fshz`.
//!
//! Permite que a gravação do `.fshz` aconteça inteiramente dentro do Rust,
//! sem que nenhum byte de célula atravesse a ponte WebKit / Tauri.

use feathersheet_core::planilha::sheet::Sheet;
use feathersheet_core::planilha::workbook::Workbook;

use crate::para_workbook::estilo_para_json;
use crate::fshz_write::{
    AbaBruta, CelulaAvulsaBruta, ColunaFormulaBruta, ColunaNumBruta, ColunaTxtBruta, ErroBruto, PastaBruta,
};

/// O estilo da entrada `idx` para o arquivo. Prefere o parcial (que guarda
/// todos os campos: recuo, reduzir, vertical, bloqueio...); o `Style`
/// resolvido só cobre o que a formatação de número e o desenho usam.
fn estilo_da_entrada(sheet: &Sheet, idx: usize) -> Option<serde_json::Value> {
    match sheet.style_parts.get(idx) {
        Some(parcial) => Some(estilo_parcial_para_json(parcial)),
        None => sheet.style_table.get(idx).map(estilo_para_json),
    }
}

fn parse_json_opt(s: &Option<String>) -> Option<serde_json::Value> {
    s.as_ref().and_then(|val| serde_json::from_str(val).ok())
}

fn campo_para_json(c: &feathersheet_core::celulas::estilo_parcial::Campo) -> serde_json::Value {
    use feathersheet_core::celulas::estilo_parcial::Campo;
    match c {
        Campo::Nulo => serde_json::Value::Null,
        Campo::Logico(b) => serde_json::Value::Bool(*b),
        Campo::Numero(n) => serde_json::Number::from_f64(*n).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
        Campo::Texto(s) => serde_json::Value::String(s.clone()),
        Campo::Objeto(o) => serde_json::Value::Object(
            o.iter().map(|(k, v)| (k.clone(), campo_para_json(v))).collect(),
        ),
    }
}

pub fn estilo_parcial_para_json(e: &feathersheet_core::celulas::estilo_parcial::EstiloParcial) -> serde_json::Value {
    serde_json::Value::Object(e.iter().map(|(k, v)| (k.clone(), campo_para_json(v))).collect())
}



/// Monta a `PastaBruta` a partir da `Workbook` do Rust.
/// `ativa` é a aba que estava na frente, para reabrir nela.
pub fn da_workbook(workbook: &Workbook, ativa: u32) -> PastaBruta {
    let mut sheets = Vec::with_capacity(workbook.len());
    for i in 0..workbook.len() {
        if let Some(viva) = workbook.aba(i) {
            sheets.push(montar_aba_bruta(&viva.sheet));
        }
    }

    PastaBruta {
        a: ativa,
        names: Some(crate::para_workbook::nomes_para_json(workbook)),
        es: workbook.estilos_celula.as_deref().and_then(|t| serde_json::from_str(t).ok()),
        pr: crate::para_workbook::propriedades_para_json(&workbook.propriedades),
        mc: workbook.macros.as_deref().and_then(|t| serde_json::from_str(t).ok()),
        ex: crate::externos_guardados::para_json(&workbook.externos_para_gravar()),
        fm: workbook.formularios.as_deref().and_then(|t| serde_json::from_str(t).ok()),
        sheets,
    }
}

/// Gera o conteúdo binário compactado (`.fshz`) diretamente da `Workbook`.
pub fn pasta_gravar_bytes(workbook: &Workbook, ativa: u32) -> Result<Vec<u8>, String> {
    let pasta = da_workbook(workbook, ativa);
    crate::fshz_write::gerar_fshz2(&pasta)
}

/// Salva o arquivo binário direto no compressor/temporário atômico.
pub fn pasta_gravar_arquivo(workbook: &Workbook, ativa: u32, path: &std::path::Path) -> Result<(), String> {
    let pasta = da_workbook(workbook, ativa);
    crate::arquivo::gravar_atomico_com(path, |arquivo| {
        crate::fshz_write::gravar_fshz_para(arquivo, &pasta).map_err(std::io::Error::other)
    }).map_err(|e| e.to_string())
}

fn montar_aba_bruta(sheet: &Sheet) -> AbaBruta {
    let me = sheet.used_rect();
    let max_row = if sheet.cells.is_empty() && sheet.data.size() == 0 {
        sheet.max_row
    } else {
        me.r1
    };

    // A fórmula de padrão mora na `FormulaTable`: sem olhar para ela, uma aba
    // só com fórmula na linha 1 contava como vazia e a fórmula sumia.
    let linhas = if max_row == 0 && sheet.cells.is_empty() && sheet.data.size() == 0 && sheet.formulas.size() == 0 {
        0
    } else {
        max_row + 1
    };

    // 1) Células avulsas (fórmulas e soltas)
    let mut avulsas = Vec::new();
    let mut keys: Vec<_> = sheet.cells.keys().cloned().collect();
    keys.sort_unstable();

    for k in keys {
        if let Some(cell) = sheet.cells.get(&k) {
            let estilo = cell.style_idx.and_then(|idx| estilo_da_entrada(sheet, idx));
            avulsas.push(CelulaAvulsaBruta {
                k,
                v: cell.raw.clone(),
                estilo,
            });
        }
    }

    // 2) Estilo direto das células literais e das fórmulas empacotadas, por
    //    faixa — como a aba guarda. O JSON de cada estilo é montado uma vez
    //    por entrada da tabela, não uma vez por célula (eram 6 milhões de
    //    clones na Planilha Pesada: 12 GB e 3,3 s a cada Ctrl+S).
    let mut json_por_estilo: Vec<Option<Option<serde_json::Value>>> = vec![None; sheet.style_parts.len().max(sheet.style_table.len())];
    let mut estilos_faixa = Vec::with_capacity(sheet.styles.faixas_len());
    for (col, faixa) in sheet.styles.faixas() {
        let idx = faixa.idx as usize;
        if idx >= json_por_estilo.len() {
            json_por_estilo.resize(idx + 1, None);
        }
        let json = json_por_estilo[idx].get_or_insert_with(|| estilo_da_entrada(sheet, idx));
        if let Some(json) = json {
            estilos_faixa.push((col, faixa.r0, faixa.r1, json.clone()));
        }
    }

    // 3) Colunas numéricas e de texto
    let colunas_num = sheet.data.retratos_num()
        .into_iter()
        .map(|(col, blocos)| ColunaNumBruta { col, values: vec![], bitmap: vec![], blocos: Some(blocos) })
        .collect();

    // O dicionário de cada coluna de texto é uma passada por linha: uma
    // coluna por núcleo (1,7 s em 13,7 milhões de linhas, 18/10/2026).
    let (dados, pool) = (&sheet.data, &sheet.pool);
    let colunas_txt = feathersheet_core::paralelismo::por_coluna(&dados.colunas_txt(), |col| {
            let (mut indices, bitmap) = dados.exportar_coluna_txt(col, linhas).unwrap_or_default();
            let dict = crate::fshz_write::dicionario_local(&mut indices, &bitmap, |id| pool.get(id))
                .into_iter().map(str::to_owned).collect();
            ColunaTxtBruta { col, indices, bitmap, dict }
        });

    // 4) Colunas de fórmula empacotada (padrão + valor calculado). Elas não
    //    moram em `cells`: esquecê-las aqui apagava a coluna ao salvar.
    let colunas_formula = sheet
        .formulas
        .columns_for_save()
        .into_iter()
        .filter_map(|c| {
            let salva = sheet.formulas.save_column(c.col, linhas, &sheet.pool)?;
            Some(ColunaFormulaBruta {
                col: c.col,
                slots: salva.slots,
                payload: salva.payload,
                bitmap: salva.bitmap,
                dict: salva.dict,
                erros: salva.erros.into_iter().map(|e| ErroBruto { code: e.code, detail: e.detail }).collect(),
            })
        })
        .collect();

    // 5) Larguras de coluna e alturas de linha
    let mut w: Vec<(u32, u32)> = sheet.col_widths.iter().map(|(&c, &w)| (c, w)).collect();
    w.sort_by_key(|(c, _)| *c);

    let mut h: Vec<(u32, u32)> = sheet.row_heights.iter().map(|(&r, &h)| (r, h)).collect();
    h.sort_by_key(|(r, _)| *r);

    AbaBruta {
        n: sheet.name.clone(),
        tc: sheet.tab_color.clone(),
        oc: sheet.oculta,
        wp: sheet.largura_padrao,
        tp: topicos_para_json(&sheet.topicos),
        pt: sheet.protecao.as_ref().map(protecao_para_json),
        cm: (!sheet.comentarios.is_empty()).then(|| comentarios_para_json(&sheet.comentarios)),
        lo: sheet.linhas_ocultas_desde,
        co: sheet.colunas_ocultas_desde,
        w,
        h,
        g: parse_json_opt(&sheet.g),
        pr: parse_json_opt(&sheet.pr),
        r: parse_json_opt(&sheet.r),
        tb: parse_json_opt(&sheet.tb),
        oq: parse_json_opt(&sheet.oq),
        dn: parse_json_opt(&sheet.dn),
        mc: parse_json_opt(&sheet.mc),
        im: parse_json_opt(&sheet.im),
        pd: crate::pendentes::pendentes_da_aba(sheet),
        va: parse_json_opt(&sheet.va),
        bt: parse_json_opt(&sheet.bt),
        tn: parse_json_opt(&sheet.tn),
        ob: parse_json_opt(&sheet.ob),
        ce: parse_json_opt(&sheet.ce),
        avulsas,
        estilos_literais: Vec::new(),
        estilos_faixa,
        estilos_coluna: sheet.col_styles.iter().map(|(k, v)| (*k, estilo_parcial_para_json(v))).collect(),
        estilos_linha: sheet.row_styles.iter().map(|(k, v)| (*k, estilo_parcial_para_json(v))).collect(),
        estilo_aba: sheet.sheet_style.as_ref().map(estilo_parcial_para_json),
        linhas,
        colunas_num,
        colunas_txt,
        colunas_formula,
        padroes_formula: sheet.formulas.pattern_list().to_vec(),
    }
}

/// A estrutura de tópicos no `.fshz`: `{"l": [[linha, nível]…], "c": […],
/// "rl": [resumos recolhidos], "rc": […]}`. `None` quando não há nada.
pub fn topicos_para_json(t: &feathersheet_core::planilha::sheet::topicos::Topicos) -> Option<serde_json::Value> {
    use serde_json::json;
    if t.vazio() && t.linhas.recolhidas.is_empty() && t.colunas.recolhidas.is_empty() { return None; }
    let pares = |m: &std::collections::BTreeMap<u32, u8>| m.iter().map(|(&i, &n)| json!([i, n])).collect::<Vec<_>>();
    Some(json!({
        "l": pares(&t.linhas.niveis), "c": pares(&t.colunas.niveis),
        "rl": t.linhas.recolhidas.iter().collect::<Vec<_>>(),
        "rc": t.colunas.recolhidas.iter().collect::<Vec<_>>(),
    }))
}

/// Os comentários da aba no `.fshz`: `[{r, c, a, t, v}]` (`v` só quando
/// o balão fica sempre à mostra).
pub fn comentarios_para_json(m: &feathersheet_core::planilha::sheet::comentarios::Comentarios) -> serde_json::Value {
    serde_json::Value::Array(m.iter().map(|(&(r, c), cm)| {
        let mut item = serde_json::json!({ "r": r, "c": c, "a": cm.autor, "t": cm.texto });
        if cm.visivel { item["v"] = serde_json::Value::Bool(true); }
        item
    }).collect())
}

/// A proteção da aba no `.fshz` (e no meio do caminho do `.xlsx`).
pub fn protecao_para_json(p: &feathersheet_core::planilha::sheet::protecao::ProtecaoPlanilha) -> serde_json::Value {
    serde_json::json!({
        "senha": p.senha.as_ref().map(crate::para_workbook::senha_para_json),
        "p": p.permitidas.iter().map(|x| x.chave()).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ida_e_volta_workbook_fshz() {
        let mut wb = Workbook::vazia();
        wb.definir_nome("teste", feathersheet_core::celulas::refs::Rect::new(0, 0, 0, 0));
        let idx = wb.criar_aba("MinhaAba");
        let esperava_g_val = serde_json::json!([{"id": "grafico1"}]);
        let esperava_g_str = Some(esperava_g_val.to_string());
        let esperava_col_styles_val = serde_json::json!({"bold": true});
        let mut col_styles = std::collections::HashMap::new();
        col_styles.insert(0, crate::para_workbook::estilo_parcial_de_json(&esperava_col_styles_val).unwrap());

        {
            let viva = wb.aba_mut(idx).unwrap();
            viva.sheet.g = esperava_g_str;
            viva.sheet.col_styles = col_styles;
            viva.sheet.set_cell_raw(0, 0, "123.45".to_string(), None);
            viva.sheet.set_cell_raw(1, 0, "=A1*2".to_string(), None);
        }

        let bytes = pasta_gravar_bytes(&wb, 0).expect("deveria gravar o fshz sem erros");
        assert!(!bytes.is_empty());

        let lida = crate::fshz_read::ler_fshz(&bytes).expect("deveria ler o fshz gravado");
        assert_eq!(
            lida.names,
            Some(serde_json::json!([{ "name": "teste", "rect": { "r0": 0, "c0": 0, "r1": 0, "c1": 0 } }]))
        );
        assert_eq!(lida.sheets.len(), 1);
        assert_eq!(lida.sheets[0].n, "MinhaAba");
        assert_eq!(lida.sheets[0].g, Some(esperava_g_val));
        assert_eq!(lida.sheets[0].estilos_coluna, vec![(0, esperava_col_styles_val)]);
    }
    /// A fórmula que virou padrão mora na `FormulaTable`, não em `cells`, e
    /// até 23/09/2026 o gravador só olhava `cells`: abrir um `.fshz` com uma
    /// coluna de fórmula e salvar de novo apagava a coluna inteira.
    #[test]
    fn a_coluna_de_formula_sobrevive_a_gravar_e_abrir() {
        // 200 linhas: aba pequena, tudo vira avulsa. 300: vai em Arrow.
        for n in [200u32, 300] {
            let mut wb = Workbook::vazia();
            let idx = wb.criar_aba("Contas");
            {
                let s = &mut wb.aba_mut(idx).unwrap().sheet;
                for r in 0..n {
                    s.set_cell_raw(r, 0, (r + 1).to_string(), None);
                    s.set_cell_raw(r, 1, format!("=A{}*2", r + 1), None);
                }
                assert!(s.formulas.size() > 0, "o teste precisa de fórmula na tabela colunar");
            }
            let bytes = pasta_gravar_bytes(&wb, 0).expect("grava");
            let lida = crate::fshz_read::ler_fshz(&bytes).expect("lê");
            let volta = crate::para_workbook::para_workbook(&lida);
            let s = &volta.aba(0).unwrap().sheet;
            assert_eq!(s.get_raw(10, 1), "=A11*2", "{n} linhas");
            assert_eq!(s.get_raw(n - 1, 1), format!("=A{n}*2"), "{n} linhas");
            assert_eq!(s.get_value(10, 0), feathersheet_core::celulas::values::Value::Number(11.0), "{n} linhas");
            assert_eq!(s.get_value(10, 1), feathersheet_core::celulas::values::Value::Number(22.0), "{n} linhas");
        }
    }
    /// Uma aba cujo único conteúdo é fórmula de padrão na linha 1 (`=1+1`,
    /// `=BUSCARTEXTO("…")`): até 26/09/2026 a aba contava como vazia (só
    /// `cells` e `data` eram olhados), gravava zero linhas, e a fórmula sumia
    /// do arquivo.
    /// Estilo direto por faixa (Fase 2 do plano de 400 MB): uma coluna de
    /// 300 células em moeda é UMA faixa no arquivo (`sf`), não 300 entradas,
    /// e volta igual — inclusive onde a faixa tem buraco e estilo trocado.
    #[test]
    fn estilo_por_faixa_vai_e_volta_pelo_fshz() {
        use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
        let mut wb = Workbook::vazia();
        let idx = wb.criar_aba("Vendas");
        let mut moeda = EstiloParcial::new();
        moeda.insert("format".into(), Campo::Texto("currency".into()));
        let mut negrito = EstiloParcial::new();
        negrito.insert("bold".into(), Campo::Logico(true));
        {
            let s = &mut wb.aba_mut(idx).unwrap().sheet;
            let i_moeda = s.registrar_parcial(moeda.clone()).unwrap();
            let i_negrito = s.registrar_parcial(negrito.clone()).unwrap();
            for r in 0..300u32 {
                s.set_cell_raw(r, 0, (r + 1).to_string(), Some(i_moeda));
            }
            s.set_cell_raw(150, 0, "151".to_string(), Some(i_negrito));
            s.set_cell_raw(200, 0, String::new(), None);
            // Célula só com estilo, sem valor.
            s.set_style_idx(400, 2, i_negrito);
            assert_eq!(s.styles.len(), 300);
            assert_eq!(s.styles.faixas_len(), 5, "0..149 moeda · 150 negrito · 151..199 moeda · 201..299 moeda · 400 negrito");
        }
        let bruta = da_workbook(&wb, 0);
        assert_eq!(bruta.sheets[0].estilos_faixa.len(), 5);
        assert!(bruta.sheets[0].estilos_literais.is_empty());
        let bytes = pasta_gravar_bytes(&wb, 0).expect("grava");
        let lida = crate::fshz_read::ler_fshz(&bytes).expect("lê");
        assert_eq!(lida.sheets[0].estilos_faixa.len(), 5);
        assert!(lida.sheets[0].estilos_diretos.is_empty());
        let volta = crate::para_workbook::para_workbook(&lida);
        let s = &volta.aba(0).unwrap().sheet;
        assert_eq!(s.styles.len(), 300);
        assert_eq!(s.styles.faixas_len(), 5);
        let parcial = |r: u32, c: u32| s.get_style_idx(r, c).and_then(|i| s.style_parts.get(i)).cloned();
        assert_eq!(parcial(0, 0), Some(moeda.clone()));
        assert_eq!(parcial(149, 0), Some(moeda.clone()));
        assert_eq!(parcial(150, 0), Some(negrito.clone()));
        assert_eq!(parcial(200, 0), None);
        assert_eq!(parcial(299, 0), Some(moeda.clone()));
        assert_eq!(parcial(300, 0), None);
        assert_eq!(parcial(400, 2), Some(negrito));
        assert_eq!(s.get_value(150, 0), feathersheet_core::celulas::values::Value::Number(151.0));
    }

    #[test]
    fn formula_sozinha_na_primeira_linha_sobrevive() {
        let mut wb = Workbook::vazia();
        let idx = wb.criar_aba("Só fórmula");
        {
            let s = &mut wb.aba_mut(idx).unwrap().sheet;
            s.set_cell_raw(0, 0, "=1+1".to_string(), None);
            s.set_cell_raw(0, 1, "=A1*3".to_string(), None);
            assert!(s.cells.is_empty() && s.data.size() == 0, "o teste precisa só da tabela de fórmulas");
        }
        let bytes = pasta_gravar_bytes(&wb, 0).expect("grava");
        let volta = crate::para_workbook::para_workbook(&crate::fshz_read::ler_fshz(&bytes).expect("lê"));
        let s = &volta.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(0, 0), "=1+1");
        assert_eq!(s.get_raw(0, 1), "=A1*3");
        assert_eq!(s.get_value(0, 1), feathersheet_core::celulas::values::Value::Number(6.0));
    }
    /// O nome vivo é o que vai para o arquivo — e o que volta dele. Até
    /// 23/09/2026 o gravador usava um JSON opaco guardado na abertura: um
    /// nome criado no aplicativo não era gravado, e um apagado voltava.
    #[test]
    fn os_nomes_vivos_vao_e_voltam() {
        let mut wb = Workbook::vazia();
        wb.criar_aba("Dados");
        wb.definir_nome("Total", feathersheet_core::celulas::refs::Rect::new(0, 1, 9, 1));
        let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        let volta = crate::para_workbook::para_workbook(&lida);
        let nomes: Vec<String> = volta.nomeados().into_iter().map(|n| n.nome).collect();
        assert_eq!(nomes, vec!["Total".to_string()]);
        assert_eq!(volta.nomeados()[0].area, feathersheet_core::celulas::refs::Rect::new(0, 1, 9, 1));
    }
    #[test]
    fn ocultacao_de_aba_sobrevive_roundtrip_fshz_sem_perder_celulas() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "123".into());
        let outra = wb.criar_aba("Resumo");
        assert_eq!(wb.ocultar_aba(0), Ok(outra));
        let bytes = pasta_gravar_bytes(&wb, outra as u32).unwrap();
        let leitura = crate::fshz_read::ler_fshz(&bytes).unwrap();
        let copia = crate::para_workbook::para_workbook(&leitura);
        assert!(copia.aba(0).unwrap().sheet.oculta);
        assert!(!copia.aba(1).unwrap().sheet.oculta);
        assert_eq!(copia.valor(0, 0, 0), feathersheet_core::Value::Number(123.0));
    }

    #[test]
    fn ocultar_dimensoes_persiste_no_fshz_sem_perder_celulas() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 8, 3, "123".into());
        wb.editar(0, |sheet| {
            assert_eq!(sheet.alterar_visibilidade_dimensoes(false, true, 8, 9), Ok(2));
            assert_eq!(sheet.alterar_visibilidade_dimensoes(true, true, 3, 3), Ok(1));
        });
        let bytes = pasta_gravar_bytes(&wb, 0).expect("grava fshz");
        let lida = crate::fshz_read::ler_fshz(&bytes).expect("lê fshz");
        let restaurada = crate::para_workbook::para_workbook(&lida);
        let aba = &restaurada.aba(0).unwrap().sheet;
        assert_eq!(aba.row_heights.get(&8), Some(&0));
        assert_eq!(aba.row_heights.get(&9), Some(&0));
        assert_eq!(aba.col_widths.get(&3), Some(&0));
        assert_eq!(aba.get_raw(8, 3), "123");
    }

    #[test]
    fn ocultas_ate_o_fim_sobrevivem_fshz_e_xlsx() {
        use feathersheet_core::{MAX_COLS, MAX_ROWS};
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 2, 0, "visível".into());
        wb.escrever_celula(0, 40, 0, "escondida".into());
        wb.editar(0, |s| {
            s.alterar_visibilidade_dimensoes(false, true, 29, MAX_ROWS - 1).unwrap();
            s.alterar_visibilidade_dimensoes(true, true, 3, MAX_COLS - 1).unwrap();
        });
        let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        let restaurada = crate::para_workbook::para_workbook(&lida);
        let aba = &restaurada.aba(0).unwrap().sheet;
        assert_eq!((aba.linhas_ocultas_desde, aba.colunas_ocultas_desde), (Some(29), Some(3)));
        assert_eq!(aba.get_raw(40, 0), "escondida");

        let xlsx = crate::xlsx_write::gerar_xlsx(&wb).expect("grava xlsx");
        let (de_volta, _) = crate::xlsx_read::ler_xlsx_workbook(xlsx).expect("lê xlsx");
        let aba = &de_volta.aba(0).unwrap().sheet;
        assert_eq!((aba.linhas_ocultas_desde, aba.colunas_ocultas_desde), (Some(29), Some(3)));
        assert!(!aba.linha_oculta(10) && aba.linha_oculta(40) && aba.linha_oculta(1_048_575));
        assert!(aba.col_widths.is_empty(), "sem uma largura por coluna oculta");
        assert_eq!(aba.get_raw(40, 0), "escondida");
    }

    #[test]
    fn recuo_sobrevive_fshz_e_xlsx() {
        use feathersheet_core::celulas::refs::Rect;
        use feathersheet_core::operacoes::recuo::{alterar_recuo, recuo_de};
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "texto".into());
        wb.escrever_celula(0, 1, 0, "outro".into());
        wb.editar(0, |s| {
            alterar_recuo(s, &Rect::cell(0, 0), 3);
            alterar_recuo(s, &Rect::cell(1, 0), 1);
        });
        let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        let fshz = crate::para_workbook::para_workbook(&lida);
        assert_eq!(recuo_de(&fshz.aba(0).unwrap().sheet, 0, 0), 3);
        assert_eq!(recuo_de(&fshz.aba(0).unwrap().sheet, 1, 0), 1);
        let bytes = crate::xlsx_write::gerar_xlsx(&wb).unwrap();
        let (xlsx, _) = crate::xlsx_read::ler_xlsx_workbook(bytes).unwrap();
        let aba = &xlsx.aba(0).unwrap().sheet;
        assert_eq!(recuo_de(aba, 0, 0), 3);
        assert_eq!(recuo_de(aba, 1, 0), 1);
        assert_eq!(recuo_de(aba, 2, 0), 0);
    }

    #[test]
    fn comentarios_sobrevivem_fshz_e_xlsx() {
        use feathersheet_core::planilha::sheet::comentarios::Comentario;
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "10".into());
        wb.editar(0, |s| {
            s.definir_comentario(0, 0, Some(Comentario { autor: "Ana Lima".into(), texto: "Conferir <total> & \"imposto\"\nsegunda linha".into(), visivel: false }));
            s.definir_comentario(4, 27, Some(Comentario { autor: String::new(), texto: "sem autor, à mostra".into(), visivel: true }));
            s.definir_comentario(2, 1, Some(Comentario { autor: "Bia".into(), texto: String::new(), visivel: false }));
        });
        let esperado = wb.aba(0).unwrap().sheet.comentarios.clone();
        let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        let restaurada = crate::para_workbook::para_workbook(&lida);
        assert_eq!(restaurada.aba(0).unwrap().sheet.comentarios, esperado);
        let bytes = crate::xlsx_write::gerar_xlsx(&wb).unwrap();
        let (de_volta, _) = crate::xlsx_read::ler_xlsx_workbook(bytes).unwrap();
        assert_eq!(de_volta.aba(0).unwrap().sheet.comentarios, esperado);
    }

    #[test]
    fn protecao_sobrevive_fshz_e_xlsx() {
        use feathersheet_core::planilha::sheet::protecao::{Permissao, ProtecaoPlanilha, SenhaPlanilha};
        use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
        use feathersheet_core::celulas::refs::Rect;
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "=1+1".into());
        wb.editar(0, |s| {
            let mut e = EstiloParcial::new();
            e.insert("locked".into(), Campo::Logico(false));
            s.set_style(&Rect::cell(1, 1), &e);
            let mut o = EstiloParcial::new();
            o.insert("hidden".into(), Campo::Logico(true));
            s.set_style(&Rect::cell(0, 0), &o);
            let mut permitidas = Permissao::padrao();
            permitidas.insert(Permissao::Classificar);
            s.proteger(ProtecaoPlanilha {
                senha: Some(SenhaPlanilha { algoritmo: "SHA-512".into(), hash: "aGFzaA==".into(), sal: "c2Fs".into(), giros: 100_000 }),
                permitidas,
            });
        });
        let esperado = wb.aba(0).unwrap().sheet.protecao.clone();
        let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        let restaurada = crate::para_workbook::para_workbook(&lida);
        assert_eq!(restaurada.aba(0).unwrap().sheet.protecao, esperado);
        let bytes = crate::xlsx_write::gerar_xlsx(&wb).unwrap();
        let (de_volta, _) = crate::xlsx_read::ler_xlsx_workbook(bytes).unwrap();
        let aba = &de_volta.aba(0).unwrap().sheet;
        assert_eq!(aba.protecao, esperado);
        assert!(!aba.celula_bloqueada(1, 1) && aba.celula_bloqueada(1, 0));
        assert!(aba.formula_oculta(0, 0), "fórmula com estilo direto mantém o estilo no .xlsx");
    }

    #[test]
    fn topicos_sobrevivem_fshz_e_xlsx() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "x".into());
        wb.editar(0, |s| {
            s.agrupar(false, 1, 4).unwrap();
            s.agrupar(false, 2, 3).unwrap();
            s.alternar_grupo(false, 2, 4).unwrap();
            s.agrupar(true, 2, 3).unwrap();
        });
        let esperado = wb.aba(0).unwrap().sheet.topicos.clone();
        let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        let restaurada = crate::para_workbook::para_workbook(&lida);
        assert_eq!(restaurada.aba(0).unwrap().sheet.topicos, esperado);
        let xlsx = crate::xlsx_write::gerar_xlsx(&wb).expect("grava xlsx");
        let (de_volta, _) = crate::xlsx_read::ler_xlsx_workbook(xlsx).expect("lê xlsx");
        let aba = &de_volta.aba(0).unwrap().sheet;
        assert_eq!(aba.topicos, esperado);
        assert_eq!(aba.row_heights.get(&2), Some(&0), "o recolhido continua oculto");
    }

    #[test]
    fn largura_padrao_sobrevive_fshz_e_xlsx() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "x".into());
        wb.editar(0, |s| { assert!(s.definir_largura_padrao(Some(150))); });
        let bytes = pasta_gravar_bytes(&wb, 0).expect("grava fshz");
        let lida = crate::fshz_read::ler_fshz(&bytes).expect("lê fshz");
        let restaurada = crate::para_workbook::para_workbook(&lida);
        assert_eq!(restaurada.aba(0).unwrap().sheet.largura_padrao, Some(150));
        let xlsx = crate::xlsx_write::gerar_xlsx(&wb).expect("grava xlsx");
        let (de_volta, _) = crate::xlsx_read::ler_xlsx_workbook(xlsx).expect("lê xlsx");
        assert_eq!(de_volta.aba(0).unwrap().sheet.largura_padrao, Some(150));
        // os estilos de célula da pessoa vão no .fshz
        let mut wb = wb;
        wb.estilos_celula = Some(r#"[{"nome":"Meu","estilo":{"bold":true}}]"#.into());
        let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        let restaurada = crate::para_workbook::para_workbook(&lida);
        let v: serde_json::Value = serde_json::from_str(restaurada.estilos_celula.as_deref().unwrap()).unwrap();
        assert_eq!(v[0]["nome"], "Meu");
        // sem largura padrão própria, o .xlsx declara a do motor (122 px)
        let vazia = Workbook::new();
        let (de_volta, _) = crate::xlsx_read::ler_xlsx_workbook(crate::xlsx_write::gerar_xlsx(&vazia).unwrap()).unwrap();
        assert_eq!(de_volta.aba(0).unwrap().sheet.largura_padrao, Some(122));
    }

    #[test]
    fn propriedades_do_arquivo_vao_e_voltam_no_fshz_e_no_xlsx() {
        use feathersheet_core::planilha::workbook::Propriedades;
        let mut wb = Workbook::new();
        // Sem autor, nenhum dos dois formatos ganha o campo.
        let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        assert!(lida.pr.is_none());
        let xlsx = crate::xlsx_write::gerar_xlsx(&wb).unwrap();
        assert!(!String::from_utf8_lossy(&xlsx).contains("docProps/core.xml"));
        wb.propriedades = Propriedades {
            autor: "Ana & Cia".into(), modificado_por: "João".into(), organizacao: "Feather <Ltda>".into(),
            protecao_estrutura: None,
        };
        let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        assert_eq!(crate::para_workbook::para_workbook(&lida).propriedades, wb.propriedades);
        let xlsx = crate::xlsx_write::gerar_xlsx(&wb).unwrap();
        let (de_volta, _) = crate::xlsx_read::ler_xlsx_workbook(xlsx).unwrap();
        assert_eq!(de_volta.propriedades, wb.propriedades);
    }

    #[test]
    fn protecao_da_pasta_vai_e_volta_no_fshz_e_no_xlsx() {
        use feathersheet_core::planilha::{sheet::protecao::SenhaPlanilha, workbook::ProtecaoPasta};
        let senha = SenhaPlanilha { algoritmo: "SHA-512".into(), hash: "aGFzaA==".into(), sal: "c2Fs".into(), giros: 100_000 };
        for protecao in [ProtecaoPasta { senha: None }, ProtecaoPasta { senha: Some(senha) }] {
            let mut wb = Workbook::new();
            wb.propriedades.protecao_estrutura = Some(protecao);
            let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
            assert_eq!(crate::para_workbook::para_workbook(&lida).propriedades, wb.propriedades);
            let (de_volta, _) = crate::xlsx_read::ler_xlsx_workbook(crate::xlsx_write::gerar_xlsx(&wb).unwrap()).unwrap();
            assert_eq!(de_volta.propriedades.protecao_estrutura, wb.propriedades.protecao_estrutura);
        }
    }

    #[test]
    fn macros_da_pasta_vao_e_voltam_no_fshz() {
        let mut wb = Workbook::new();
        let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        assert!(lida.mc.is_none());
        wb.macros = Some(r#"{"macros":[{"name":"Colorir","code":"sheet.set('A1',1)","auto":false}]}"#.into());
        let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        let v: serde_json::Value = serde_json::from_str(crate::para_workbook::para_workbook(&lida).macros.as_deref().unwrap()).unwrap();
        assert_eq!(v["macros"][0]["name"], "Colorir");
    }

    #[test]
    fn formularios_da_pasta_vao_e_voltam_no_fshz() {
        let mut wb = Workbook::new();
        assert!(crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap().fm.is_none());
        wb.formularios = Some(r#"[{"nome":"cadastro","controles":[]}]"#.into());
        let lida = crate::fshz_read::ler_fshz(&pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        let v: serde_json::Value = serde_json::from_str(crate::para_workbook::para_workbook(&lida).formularios.as_deref().unwrap()).unwrap();
        assert_eq!(v[0]["nome"], "cadastro");
    }

    /// Medida, não roda no `cargo test`: `cargo test --release -p sheet-io
    /// medir_gravacao -- --ignored --nocapture`. Pasta de 2 milhões de linhas
    /// × 6 colunas (3 de número, 3 de texto), como a `planilha_2m`.
    #[test]
    #[ignore]
    fn medir_gravacao() {
        fn pico_mb() -> u64 {
            let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
                .unwrap_or(0)
                / 1024
        }
        let n = 2_000_000u32;
        let mut wb = Workbook::vazia();
        let idx = wb.criar_aba("planilha_2m");
        {
            let s = &mut wb.aba_mut(idx).unwrap().sheet;
            for r in 0..n {
                s.set_cell_valor(r, 0, feathersheet_core::celulas::values::Value::Number(r as f64));
                s.set_cell_valor(r, 1, feathersheet_core::celulas::values::Value::Number((r % 1000) as f64 * 1.5));
                s.set_cell_valor(r, 2, feathersheet_core::celulas::values::Value::Number((r % 77) as f64));
                s.set_cell_valor(r, 3, feathersheet_core::celulas::values::Value::Text(["ALTO", "BAIXO", "NORMAL"][(r % 3) as usize].into()));
                s.set_cell_valor(r, 4, feathersheet_core::celulas::values::Value::Text(format!("cliente {}", r % 50_000)));
                s.set_cell_valor(r, 5, feathersheet_core::celulas::values::Value::Text(format!("id-{r}")));
            }
        }
        let antes = pico_mb();
        let t = std::time::Instant::now();
        let retrato = da_workbook(&wb, 0);
        let t_retrato = t.elapsed().as_millis();
        let bytes = crate::fshz_write::gerar_fshz2(&retrato).unwrap();
        println!(
            "MEDIDA gravar: retrato {t_retrato} ms, total {} ms, {} MB no disco, pico {} MB (antes {} MB)",
            t.elapsed().as_millis(),
            bytes.len() / 1_048_576,
            pico_mb(),
            antes
        );
    }
}
