//! Tabela dinâmica vai e volta do `.xlsx`: gravar a definição com o
//! `xlsx_write` (cache + tabela dinâmica, `refreshOnLoad`), reler com o
//! `xlsx_read` e encontrar os mesmos campos e opções.
use std::collections::BTreeMap;

use feathersheet_core::dinamica::exibicao::{CampoBase, CampoExtra, Eixo, Extras, ItemBase, MostrarComo, Ordenacao};
use feathersheet_core::celulas::values::Value;
use feathersheet_core::planilha::workbook::Workbook;
use feathersheet_io::xlsx_read::ler_xlsx_workbook;
use feathersheet_io::xlsx_write::{gerar_xlsx, montar_partes};
use serde_json::{json, Value as Json};

const DADOS: [[&str; 4]; 7] = [
    ["Região", "Produto", "Ano", "Valor"],
    ["Sul", "A", "2024", "10"],
    ["Sul", "B", "2024", "20"],
    ["Norte", "A", "2025", "30"],
    ["Norte", "B", "2025", "5"],
    ["Leste", "A", "2024", "7"],
    ["Norte", "C", "2025", "1"],
];

/// "Dados" com a origem (A1:D7) e "Resumo", a aba de destino das dinâmicas.
fn pasta() -> Workbook {
    // `Workbook::new()` já traz a "Planilha1": vira "Dados".
    let mut w = Workbook::new();
    assert!(w.renomear_aba(0, "Dados"));
    w.criar_aba("Resumo");
    for (r, linha) in DADOS.iter().enumerate() {
        for (c, t) in linha.iter().enumerate() {
            let _ = w.escrever_celula(0, r as u32, c as u32, t.to_string());
        }
    }
    w
}

/// O JSON que o painel grava em `sheet.dn` (`criar_dinamica_nativa` +
/// `gravar_campos`/`gravar_opcoes`), com as chaves de `troca` por cima.
fn meta(troca: Json) -> Json {
    let mut m = json!({
        "id": "dinamica_nativa_1_1",
        "fonte": {"aba": "Dados", "area": {"r0": 0, "r1": 6, "c0": 0, "c1": 3}},
        "destino": {"row": 0, "col": 0},
        "linhas": [0], "colunas": [], "valores": [{"col": 3, "como": "soma"}], "datas": [],
        "areaResultado": {"r0": 0, "c0": 0, "r1": 4, "c1": 1},
        "layout": "compacto", "repetirRotulos": false, "subtotais": false,
        "tema": "padrao", "recolhidos": [], "filtros": [], "calculados": [],
        "extras": Extras {
            valores: vec![MostrarComo::Normal],
            linhas: vec![CampoExtra::default()],
            ..Extras::default()
        },
    });
    for (k, v) in troca.as_object().expect("objeto") {
        m[k] = v.clone();
    }
    m
}

fn com_dinamica(m: &Json) -> Workbook {
    let mut w = pasta();
    w.aba_mut(1).unwrap().sheet.dn = Some(json!([m]).to_string());
    w
}

/// As partes do `.xlsx` gravado, por nome.
fn partes(w: &Workbook) -> BTreeMap<String, String> {
    montar_partes(w).into_iter().map(|(n, b)| (n, String::from_utf8(b).expect("UTF-8"))).collect()
}

/// Falha se o XML não for bem formado: fechamento trocado, tag sem fechar.
fn bem_formado(nome: &str, xml: &str) {
    use quick_xml::events::Event;
    let mut leitor = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut profundidade = 0i64;
    loop {
        match leitor.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(_)) => profundidade += 1,
            Ok(Event::End(_)) => profundidade -= 1,
            Ok(_) => {}
            Err(e) => panic!("{nome}: XML mal formado: {e}"),
        }
        assert!(profundidade >= 0, "{nome}: fechamento sem abertura");
        buf.clear();
    }
    assert_eq!(profundidade, 0, "{nome}: elemento sem fechar");
}

fn todas_bem_formadas(p: &BTreeMap<String, String>) {
    for (nome, xml) in p {
        if nome.ends_with(".xml") || nome.ends_with(".rels") {
            bem_formado(nome, xml);
        }
    }
}

/// Grava, relê e devolve a pasta lida e a (primeira) dinâmica de "Resumo".
fn ida_e_volta(w: &Workbook) -> (Workbook, Json) {
    let bytes = gerar_xlsx(w).expect("gravar");
    let (lida, _) = ler_xlsx_workbook(bytes).expect("ler");
    let dn = lida.aba(1).unwrap().sheet.dn.clone().expect("a dinâmica voltou do .xlsx");
    let d = serde_json::from_str::<Vec<Json>>(&dn).unwrap().remove(0);
    (lida, d)
}

fn extras(d: &Json) -> Extras {
    serde_json::from_value(d["extras"].clone()).expect("extras")
}

fn comos(d: &Json) -> Vec<String> {
    d["valores"].as_array().unwrap().iter().map(|v| v["como"].as_str().unwrap().to_string()).collect()
}

#[test]
fn dinamica_simples_vai_e_volta() {
    let original = meta(json!({}));
    let w = com_dinamica(&original);
    let p = partes(&w);
    todas_bem_formadas(&p);

    let cache = &p["xl/pivotCache/pivotCacheDefinition1.xml"];
    assert!(cache.contains("refreshOnLoad=\"1\"") && cache.contains("saveData=\"0\""), "{cache}");
    assert!(cache.contains("<worksheetSource ref=\"A1:D7\" sheet=\"Dados\"/>"), "{cache}");
    assert!(cache.contains("<cacheFields count=\"4\">"), "{cache}");
    assert!(cache.contains("<s v=\"Leste\"/><s v=\"Norte\"/><s v=\"Sul\"/>"), "{cache}");
    assert!(!p.keys().any(|n| n.contains("pivotCacheRecords")), "sem registros: o Excel atualiza ao abrir");

    let tabela = &p["xl/pivotTables/pivotTable1.xml"];
    assert!(tabela.contains("cacheId=\"1\""), "{tabela}");
    assert!(tabela.contains("<location ref=\"A1:B5\""), "{tabela}");
    assert!(tabela.contains("<pivotField axis=\"axisRow\""), "{tabela}");
    assert!(tabela.contains("<rowFields count=\"1\"><field x=\"0\"/></rowFields>"), "{tabela}");
    assert!(tabela.contains("<dataField fld=\"3\""), "{tabela}");
    assert!(!tabela.contains("<colFields"), "uma medida só: sem o campo Valores");

    assert!(p["xl/workbook.xml"].contains("<pivotCaches><pivotCache cacheId=\"1\" r:id=\"rIdCacheDinamica1\"/></pivotCaches>"));
    assert!(p["xl/_rels/workbook.xml.rels"].contains("Target=\"pivotCache/pivotCacheDefinition1.xml\""));
    assert!(p["xl/worksheets/_rels/sheet2.xml.rels"].contains("Target=\"../pivotTables/pivotTable1.xml\""));
    assert!(p["xl/pivotTables/_rels/pivotTable1.xml.rels"].contains("Target=\"../pivotCache/pivotCacheDefinition1.xml\""));
    let tipos = &p["[Content_Types].xml"];
    assert!(tipos.contains("PartName=\"/xl/pivotCache/pivotCacheDefinition1.xml\""), "{tipos}");
    assert!(tipos.contains("PartName=\"/xl/pivotTables/pivotTable1.xml\""), "{tipos}");

    let (lida, d) = ida_e_volta(&w);
    assert_eq!(d["fonte"], original["fonte"]);
    assert_eq!(d["linhas"], json!([0]));
    assert_eq!(d["colunas"], json!([]));
    assert_eq!(d["valores"].as_array().unwrap().len(), 1);
    assert_eq!(d["valores"][0]["col"], 3);
    assert_eq!(comos(&d), ["soma"]);
    assert_eq!(d["layout"], "compacto");
    assert_eq!(d["subtotais"], false);
    assert_eq!(d["filtros"], json!([]));
    assert_eq!(d["areaResultado"], original["areaResultado"]);
    assert_eq!(extras(&d), extras(&original));
    // O leitor reconheceu a dinâmica (e a calculou: a área estava vazia).
    assert_eq!(lida.valor(1, 4, 1), Value::Number(73.0));
}

#[test]
fn colunas_filtro_percentual_e_sem_totais_vao_e_voltam() {
    let extras_originais = Extras {
        valores: vec![MostrarComo::PercTotalGeral],
        linhas: vec![CampoExtra::default()],
        colunas: vec![CampoExtra::default()],
        totais_gerais_linhas: false,
        totais_gerais_colunas: false,
    };
    // A dinâmica começa na linha 4: há espaço acima para o filtro de relatório.
    let original = meta(json!({
        "linhas": [0], "colunas": [1],
        "layout": "tabular", "repetirRotulos": true,
        "filtros": [{"col": 2, "valores": ["2024"], "excluir": true}],
        "destino": {"row": 3, "col": 0},
        "areaResultado": {"r0": 3, "c0": 0, "r1": 6, "c1": 3},
        "extras": extras_originais,
    }));
    let w = com_dinamica(&original);
    let p = partes(&w);
    todas_bem_formadas(&p);

    let cache = &p["xl/pivotCache/pivotCacheDefinition1.xml"];
    assert!(
        cache.contains("<sharedItems containsSemiMixedTypes=\"0\" containsString=\"0\" containsNumber=\"1\" containsInteger=\"1\" minValue=\"2024\" maxValue=\"2025\" count=\"2\"><n v=\"2024\"/><n v=\"2025\"/></sharedItems>"),
        "{cache}"
    );
    let tabela = &p["xl/pivotTables/pivotTable1.xml"];
    for trecho in [
        "compact=\"0\" compactData=\"0\"",
        "rowGrandTotals=\"0\"",
        "colGrandTotals=\"0\"",
        "<pivotField axis=\"axisCol\"",
        "<pivotField axis=\"axisPage\"",
        "multipleItemSelectionAllowed=\"1\"",
        "<item h=\"1\" x=\"0\"/>",
        "<pageFields count=\"1\"><pageField fld=\"2\" hier=\"-1\"/></pageFields>",
        "rowPageCount=\"1\" colPageCount=\"1\"",
        "<colFields count=\"1\"><field x=\"1\"/></colFields>",
        "showDataAs=\"percentOfTotal\"",
        "fillDownLabels=\"1\"",
    ] {
        assert!(tabela.contains(trecho), "falta {trecho}: {tabela}");
    }

    let (_, d) = ida_e_volta(&w);
    assert_eq!(d["linhas"], json!([0]));
    assert_eq!(d["colunas"], json!([1]));
    assert_eq!(comos(&d), ["soma"]);
    assert_eq!(d["layout"], "tabular");
    assert_eq!(d["repetirRotulos"], true);
    assert_eq!(d["filtros"], json!([{"col": 2, "valores": ["2024"], "excluir": true}]));
    assert_eq!(extras(&d), extras(&original));
    assert_eq!((d["areaResultado"]["r0"].clone(), d["areaResultado"]["c0"].clone()), (json!(3), json!(0)));
}

#[test]
fn filtro_sem_espaco_acima_vai_nos_itens_e_volta() {
    // Dinâmica na linha 1 (como o painel cria): o Excel não tem onde pôr o
    // filtro de relatório, então os itens vão ocultos no campo, fora dos eixos.
    let original = meta(json!({
        "filtros": [{"col": 2, "valores": ["2025"], "excluir": false}],
    }));
    let w = com_dinamica(&original);
    let p = partes(&w);
    todas_bem_formadas(&p);
    let tabela = &p["xl/pivotTables/pivotTable1.xml"];
    assert!(!tabela.contains("<pageFields") && !tabela.contains("axisPage"), "{tabela}");
    assert!(tabela.contains("<item h=\"1\" x=\"0\"/><item x=\"1\"/>"), "{tabela}");

    let (_, d) = ida_e_volta(&w);
    // "Só 2025" volta como "tira 2024": o mesmo filtro.
    assert_eq!(d["filtros"], json!([{"col": 2, "valores": ["2024"], "excluir": true}]));
}

#[test]
fn campo_em_filtros_e_linhas_vai_como_linha_com_os_itens_ocultos() {
    // O painel deixa o mesmo campo em Filtros e Linhas (`paginaTambem`); o
    // Excel não (um campo é página OU linha). Vai como linha, com os itens
    // que o filtro tira ocultos: a mesma tabela, sem página repetida.
    let original = meta(json!({
        "linhas": [0],
        "filtros": [{"col": 0, "valores": ["Sul"], "excluir": true}],
        "paginaTambem": [0],
        "destino": {"row": 3, "col": 0},
        "areaResultado": {"r0": 3, "c0": 0, "r1": 6, "c1": 1},
    }));
    let w = com_dinamica(&original);
    let p = partes(&w);
    todas_bem_formadas(&p);
    let tabela = &p["xl/pivotTables/pivotTable1.xml"];
    assert!(!tabela.contains("<pageFields") && !tabela.contains("axisPage") && !tabela.contains("rowPageCount"), "{tabela}");
    assert!(tabela.contains("<pivotField axis=\"axisRow\""), "{tabela}");
    // Leste, Norte, Sul: só o Sul oculto.
    assert!(tabela.contains("<item x=\"0\"/><item x=\"1\"/><item h=\"1\" x=\"2\"/>"), "{tabela}");

    let (_, d) = ida_e_volta(&w);
    assert_eq!(d["linhas"], json!([0]));
    assert_eq!(d["filtros"], json!([{"col": 0, "valores": ["Sul"], "excluir": true}]));
}

#[test]
fn mostrar_como_com_item_base_classificacao_e_ordem_por_valor_vao_e_voltam() {
    let base = CampoBase { eixo: Eixo::Linhas, posicao: 0 };
    let extras_originais = Extras {
        valores: vec![
            MostrarComo::PercDe { campo: base, item: ItemBase::Item { nome: "Sul".into() } },
            MostrarComo::DiferencaDe { campo: base, item: ItemBase::Anterior },
            MostrarComo::ClassificarMaiorMenor { campo: base },
        ],
        linhas: vec![CampoExtra { agrupamento: None, ordenacao: Ordenacao::PorValor { medida: 0, crescente: false } }],
        ..Extras::default()
    };
    let original = meta(json!({
        "valores": [{"col": 3, "como": "soma"}, {"col": 3, "como": "media"}, {"col": 3, "como": "contagem"}],
        "areaResultado": {"r0": 0, "c0": 0, "r1": 4, "c1": 3},
        "extras": extras_originais,
    }));
    let w = com_dinamica(&original);
    let p = partes(&w);
    todas_bem_formadas(&p);
    let tabela = &p["xl/pivotTables/pivotTable1.xml"];
    for trecho in [
        "<colFields count=\"1\"><field x=\"-2\"/></colFields>",
        "showDataAs=\"percent\" baseField=\"0\" baseItem=\"2\"",
        "subtotal=\"average\" showDataAs=\"difference\" baseField=\"0\" baseItem=\"1048828\"",
        "subtotal=\"count\"",
        "pivotShowAs=\"rankDescending\"",
        "sortType=\"descending\"",
        "<autoSortScope>",
    ] {
        assert!(tabela.contains(trecho), "falta {trecho}: {tabela}");
    }

    let (_, d) = ida_e_volta(&w);
    assert_eq!(comos(&d), ["soma", "media", "contagem"]);
    assert_eq!(extras(&d), extras(&original));
}

#[test]
fn moda_e_campo_calculado_vao_como_o_excel_entende() {
    let original = meta(json!({
        "linhas": [0, -1],
        "valores": [{"col": 3, "como": "moda"}, {"col": -1, "como": "soma"}],
        "extras": Extras {
            valores: vec![MostrarComo::Normal, MostrarComo::PercTotalGeral],
            linhas: vec![CampoExtra::default(), CampoExtra::default()],
            ..Extras::default()
        },
    }));
    let (_, d) = ida_e_volta(&com_dinamica(&original));
    // O campo calculado sai; a moda, que o Excel não tem, vira soma.
    assert_eq!(d["linhas"], json!([0]));
    assert_eq!(comos(&d), ["soma"]);
    assert_eq!(extras(&d).valores, vec![MostrarComo::Normal]);
}

#[test]
fn arquivo_sem_dinamica_continua_igual() {
    let sem = partes(&pasta());
    assert!(sem.keys().all(|n| !n.to_lowercase().contains("pivot")), "{:?}", sem.keys());
    for nome in ["xl/workbook.xml", "xl/_rels/workbook.xml.rels", "[Content_Types].xml"] {
        assert!(!sem[nome].contains("pivot"), "{nome}: {}", sem[nome]);
    }
    assert!(!sem.contains_key("xl/worksheets/_rels/sheet2.xml.rels"));

    // Lista vazia de dinâmicas: exatamente as mesmas partes, byte a byte.
    let mut vazia = pasta();
    vazia.aba_mut(1).unwrap().sheet.dn = Some("[]".into());
    assert_eq!(partes(&vazia), sem);

    // Dinâmica que o Excel não representa (só campo calculado em Linhas):
    // também nada de dinâmica no pacote.
    let mut so_calculado = pasta();
    so_calculado.aba_mut(1).unwrap().sheet.dn = Some(json!([meta(json!({"linhas": [-1]}))]).to_string());
    assert_eq!(partes(&so_calculado), sem);
}
