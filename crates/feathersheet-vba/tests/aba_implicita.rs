//! `Range`/`Cells`/`Rows`/`Columns` sem qualificador: no módulo de uma aba são a PRÓPRIA aba
//! (mesmo depois de `OutraAba.Select`); em módulo comum, a ativa.

use feathersheet_vba::converter_texto;

const VBA: &str = "Sub T()\n  Outra.Select\n  Cells(1, 1).Value = 7\n  x = Cells(2, 1).Value\n  ActiveSheet.Range(\"C1\").Value = 1\nEnd Sub\n";

#[test]
fn modulo_de_aba_le_e_grava_na_propria_aba_apos_select() {
    let c = converter_texto(&[("Sheet8", &VBA.replace("Outra", "Planilha1"))], &[("Sheet8", "MAIN"), ("Planilha1", "Arranjos")]);
    let js = c.macros.iter().find(|m| m.nome == "T").expect("macro").codigo.clone();
    assert!(js.contains("Sheet8.set(\"A1\", 7);"), "{js}");
    assert!(js.contains("Sheet8.value(\"A2\")"), "{js}");
    assert!(js.contains("wb.activate(\"Arranjos\");"), "{js}");
    assert!(js.contains("Planilha1.set(\"C1\", 1);"), "ActiveSheet continua a ativa: {js}");
}

#[test]
fn modulo_comum_usa_a_aba_ativa_apos_select() {
    let c = converter_texto(&[("Módulo1", &VBA.replace("Outra", "Planilha1"))], &[("Sheet8", "MAIN"), ("Planilha1", "Arranjos")]);
    let js = c.macros.iter().find(|m| m.nome == "T").expect("macro").codigo.clone();
    assert!(js.contains("Planilha1.set(\"A1\", 7);"), "{js}");
    assert!(js.contains("Planilha1.value(\"A2\")"), "{js}");
}

/// Com a planilha real do usuário (só leitura): em Arranjo1/Arranjo2 nada sem qualificador
/// pode ir para a aba Arranjos.
#[test]
#[ignore = "depende da planilha do usuário"]
fn planilha_real_arranjos_vao_para_a_main() {
    let caminho = format!("{}/Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm", std::env::var("HOME").unwrap());
    let bytes = std::fs::read(caminho).unwrap();
    let c = feathersheet_vba::converter_xlsm(&bytes, "v6.xlsm").unwrap();
    let mut achou = 0;
    for e in c.eventos.iter().filter(|e| e.evento == "aoMudar") {
        for nome in ["Arranjo1", "Arranjo2"] {
            let ini = e.codigo.find(&format!("function {nome}(")).unwrap_or_else(|| panic!("sem {nome}"));
            let corpo = &e.codigo[ini..];
            let fim = corpo.find("\n}\n").unwrap();
            let corpo = &corpo[..fim];
            achou += 1;
            for l in corpo.lines() {
                assert!(!l.contains("Planilha1.value(") && !l.contains("Planilha1.cells") && !l.contains("Planilha1.rows"), "{nome}: {l}");
            }
        }
    }
    assert_eq!(achou, 2);
    if let Ok(d) = std::env::var("VBA_DUMP") {
        let mut t = String::new();
        for m in &c.macros { t += &format!("// ==== {}\n{}\n", m.nome, m.codigo); }
        for e in &c.eventos { t += &format!("// ==== evento {}\n{}\n", e.evento, e.codigo); }
        std::fs::write(d, t).unwrap();
    }
}

/// Executa o Arranjo1 convertido da planilha real no motor: com MAIN!E33 = 1 a tabela
/// "CONFIGURAÇÃO DOS ARRANJOS" tem de aparecer em Arranjos!B2.
#[test]
#[ignore = "depende da planilha do usuário"]
fn planilha_real_arranjo1_monta_a_tabela_em_arranjos() {
    use feathersheet_core::{celulas::values::Value, planilha::workbook::Workbook};
    let caminho = format!("{}/Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm", std::env::var("HOME").unwrap());
    let c = feathersheet_vba::converter_xlsm(&std::fs::read(caminho).unwrap(), "v6.xlsm").unwrap();
    let ev = c.eventos.iter().find(|e| e.evento == "aoMudar").expect("evento");
    let mut js: Vec<&str> = ev.codigo.lines().collect();
    while js.last().is_some_and(|l| l.trim().is_empty()) {
        js.pop();
    }
    js.pop(); // a chamada final de Worksheet_Change
    let js = format!("{}\nArranjo1(\"E33\");\n", js.join("\n"));
    let mut wb = Workbook::vazia();
    let main = wb.criar_aba("MAIN");
    let arr = wb.criar_aba("Arranjos");
    let a = wb.aba_mut(main).unwrap();
    a.sheet.set_cell_valor(32, 4, Value::Number(1.0)); // E33
    a.sheet.set_cell_valor(36, 4, Value::Text("2".into())); // E37
    if let Ok(d) = std::env::var("VBA_DUMP_EXEC") {
        std::fs::write(d, &js).unwrap();
    }
    let r = feathersheet_macros::executar(&mut wb, main, &js);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    let b2 = wb.valor(arr, 1, 1);
    assert_eq!(b2, Value::Text("CONFIGURAÇÃO DOS ARRANJOS".into()), "Arranjos!B2");
    let textos: Vec<Value> = (0..40).map(|l| wb.valor(arr, l, 1)).collect();
    println!("coluna B de Arranjos: {textos:?}");
    let todos = format!("{:?}", (0..40u32).flat_map(|l| (0..10u32).map(move |c| (l, c))).map(|(l, c)| wb.valor(arr, l, c)).collect::<Vec<_>>());
    assert!(todos.contains("ST-A1"), "{todos}");
}

/// Compara a aba Arranjos gerada pelo Arranjo1 (MAIN!E33 = valor salvo) com a salva pelo Excel.
/// Só relata as diferenças (println), não falha por elas.
#[test]
#[ignore = "depende da planilha do usuário"]
fn planilha_real_compara_arranjos_com_a_salva_pelo_excel() {
    let caminho = format!("{}/Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm", std::env::var("HOME").unwrap());
    let j = feathersheet_io::xlsx_read::ler_xlsx_bytes(std::fs::read(caminho).unwrap()).unwrap();
    let abas = j["sheets"].as_array().expect("abas");
    let mut wb = feathersheet_core::planilha::workbook::Workbook::vazia();
    let mut salvo_arranjos: std::collections::BTreeMap<(u32, u32), String> = Default::default();
    for a in abas {
        let nome = a["name"].as_str().unwrap();
        let idx = wb.criar_aba(nome);
        for (k, v) in a["cells"].as_object().unwrap() {
            let raw = v["raw"].as_str().unwrap_or("");
            if raw.is_empty() {
                continue;
            }
            let k: u64 = k.parse().unwrap();
            let (r, c) = ((k / 16384) as u32, (k % 16384) as u32);
            if nome == "Arranjos" {
                salvo_arranjos.insert((r, c), raw.to_string());
            }
            wb.aba_mut(idx).unwrap().sheet.set_cell_raw(r, c, raw.to_string(), None);
        }
    }
    let ev = feathersheet_vba::converter_xlsm(&std::fs::read(format!("{}/Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm", std::env::var("HOME").unwrap())).unwrap(), "v6.xlsm").unwrap();
    let ev = ev.eventos.iter().find(|e| e.evento == "aoMudar").unwrap().codigo.clone();
    let mut js: Vec<&str> = ev.lines().collect();
    while js.last().is_some_and(|l| l.trim().is_empty()) {
        js.pop();
    }
    js.pop();
    let main_e33 = wb.valor(0, 32, 4);
    let main_h33 = wb.valor(0, 32, 7);
    println!("MAIN E33={main_e33:?} H33={main_h33:?} E37={:?}", wb.valor(0, 36, 4));
    let js = format!("{}\nArranjo1(\"E33\");\nArranjo2(\"H33\");\n", js.join("\n"));
    let r = feathersheet_macros::executar(&mut wb, 0, &js);
    println!("erro: {:?}", r.erro);
    let arr = 1;
    let mut gerado: std::collections::BTreeMap<(u32, u32), String> = Default::default();
    for l in 0..600u32 {
        for c in 0..40u32 {
            let raw = wb.aba(arr).unwrap().sheet.get_raw(l, c);
            if !raw.is_empty() {
                gerado.insert((l, c), raw);
            }
        }
    }
    let (mut iguais, mut dif, mut so_excel, mut so_nos) = (0, Vec::new(), Vec::new(), Vec::new());
    for (k, v) in &salvo_arranjos {
        match gerado.get(k) {
            Some(g) if g == v => iguais += 1,
            Some(g) => dif.push((*k, v.clone(), g.clone())),
            None => so_excel.push((*k, v.clone())),
        }
    }
    for (k, v) in &gerado {
        if !salvo_arranjos.contains_key(k) {
            so_nos.push((*k, v.clone()));
        }
    }
    println!("Arranjos: Excel {} celulas, geradas {}, iguais {iguais}, diferentes {}, so no Excel {}, so nossas {}", salvo_arranjos.len(), gerado.len(), dif.len(), so_excel.len(), so_nos.len());
    for d in dif.iter().take(25) { println!("DIF {:?}", d); }
    for d in so_excel.iter().take(25) { println!("SO_EXCEL {:?}", d); }
    for d in so_nos.iter().take(25) { println!("SO_NOS {:?}", d); }
}

#[test]
fn matriz_de_texto_recebe_o_split_inteiro() {
    let vba = "Sub T()\nDim m() As String\nm = Split(\"2,3\", \",\")\nRange(\"A1\").Value = m(1)\nEnd Sub\n";
    let c = converter_texto(&[("Módulo1", vba)], &[]);
    let js = c.macros.iter().map(|m| m.codigo.as_str()).collect::<String>();
    assert!(js.contains("vba.split(\"2,3\", \",\")") && !js.contains("vba.str(vba.split"), "{js}");
}
