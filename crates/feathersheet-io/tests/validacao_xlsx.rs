//! Validação de dados vai e volta do `.xlsx`: gravar com o `xlsx_write`,
//! reler com o `xlsx_read` e encontrar a mesma regra.
use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::operacoes::validation::{
    DataOp, MensagensValidacao, NumeroOp, QuandoValidacao, Regra, TextoOp, Validacao,
};
use feathersheet_core::planilha::workbook::Workbook;
use feathersheet_io::xlsx_read::ler_xlsx_workbook;
use feathersheet_io::xlsx_write::{gerar_xlsx, montar_partes};

fn regras(w: &Workbook, aba: usize) -> Vec<Validacao> {
    let va = w.aba(aba).unwrap().sheet.va.clone().unwrap_or_default();
    if va.is_empty() { return Vec::new(); }
    serde_json::from_str(&va).expect("JSON das validações")
}

fn v(ranges: Vec<Rect>, regra: Regra, quando: QuandoValidacao, mensagem: Option<&str>) -> Validacao {
    Validacao { ranges, regra, quando, mensagem: mensagem.map(str::to_string), mensagens: Default::default() }
}

fn exemplo() -> Vec<Validacao> {
    use NumeroOp::*;
    let mut lista = vec![
        v(vec![Rect::new(18, 1, 18, 1)], Regra::Lista { valores: vec!["SIM".into(), "NÃO".into(), "Talvez & <sei>".into()] },
            QuandoValidacao::Recusar, Some("Escolha \"SIM\" ou <NÃO>")),
        v(vec![Rect::new(1, 2, 8, 2)], Regra::Intervalo { origem: Rect::new(1, 0, 8, 0) }, QuandoValidacao::Avisar, None),
        // Lista dependente (INDIRETO), em duas áreas.
        v(vec![Rect::new(27, 4, 27, 4), Rect::new(27, 7, 30, 7)], Regra::Formula { formula: "=INDIRETO(E28)".into() },
            QuandoValidacao::Recusar, None),
        // Outra aba, com e sem aspas no nome.
        v(vec![Rect::new(5, 3, 98, 3)], Regra::Formula { formula: "='Quadro-resumo '!$B$5:$I$5".into() },
            QuandoValidacao::Recusar, None),
        v(vec![Rect::new(0, 9, 0, 9)], Regra::Formula { formula: "=Dados!$A$2:$A$9".into() }, QuandoValidacao::Recusar, None),
        // Nome definido.
        v(vec![Rect::new(0, 10, 0, 10)], Regra::Formula { formula: "=Inversores".into() }, QuandoValidacao::Recusar, None),
        v(vec![Rect::new(2, 5, 4, 5)], Regra::Data { op: DataOp::Entre, a: "01/01/2024".into(), b: Some("31/12/2026".into()) },
            QuandoValidacao::Recusar, None),
        v(vec![Rect::new(2, 6, 2, 6)], Regra::Data { op: DataOp::Antes, a: "29/02/2000".into(), b: None }, QuandoValidacao::Avisar, None),
        v(vec![Rect::new(3, 6, 3, 6)], Regra::Data { op: DataOp::Depois, a: "15/10/2026".into(), b: None }, QuandoValidacao::Recusar, None),
    ];
    // Os oito operadores, inteiro e decimal.
    for (i, op) in [Entre, Fora, Igual, Diferente, Maior, Menor, MaiorIgual, MenorIgual].into_iter().enumerate() {
        let dois = matches!(op, Entre | Fora);
        for inteiro in [true, false] {
            let col = 20 + i as u32 * 2 + inteiro as u32;
            lista.push(v(vec![Rect::new(1, col, 50, col)],
                Regra::Numero { op, a: if inteiro { 1.0 } else { -2.5 }, b: dois.then_some(1e6), inteiro },
                QuandoValidacao::Recusar, Some("Número fora da faixa")));
        }
    }
    lista
}

#[test]
fn cada_regra_vai_e_volta_do_xlsx() {
    let mut w = Workbook::new();
    w.criar_aba("Planilha1");
    let originais = exemplo();
    w.aba_mut(0).unwrap().sheet.va = Some(serde_json::to_string(&originais).unwrap());

    let bytes = gerar_xlsx(&w).expect("gravar");
    let (lida, _) = ler_xlsx_workbook(bytes).expect("ler");
    let voltaram = regras(&lida, 0);
    assert_eq!(voltaram.len(), originais.len());
    // `INDIRETO` vai como `INDIRECT` e o leitor o traduz de volta.
    assert_eq!(voltaram, originais);
}

#[test]
fn bloco_fica_na_ordem_do_esquema_e_em_ingles() {
    let mut w = Workbook::new();
    w.criar_aba("Planilha1");
    w.escrever_celula(0, 0, 0, "x".to_string());
    w.aba_mut(0).unwrap().sheet.va = Some(serde_json::to_string(&exemplo()).unwrap());
    let partes = montar_partes(&w);
    let xml = String::from_utf8(partes.iter().find(|(n, _)| n == "xl/worksheets/sheet1.xml").unwrap().1.clone()).unwrap();
    let dv = xml.find("<dataValidations count=\"25\">").expect("bloco com a contagem");
    assert!(xml.find("</sheetData>").unwrap() < dv);
    assert!(xml.contains("<formula1>INDIRECT(E28)</formula1>"), "{xml}");
    assert!(xml.contains("<formula1>&quot;SIM,NÃO,Talvez &amp; &lt;sei&gt;&quot;</formula1>"), "{xml}");
    assert!(xml.contains("error=\"Escolha &quot;SIM&quot; ou &lt;NÃO&gt;\""), "{xml}");
    assert!(xml.contains("sqref=\"E28 H28:H31\""), "{xml}");
    assert!(xml.contains("<formula1>&apos;Quadro-resumo &apos;!$B$5:$I$5</formula1>"), "{xml}");
    assert!(xml.contains("<formula1>-2.5</formula1><formula2>1000000</formula2>"), "decimal com ponto: {xml}");
    assert!(!xml.contains("showDropDown"), "showDropDown=1 esconderia a seta");
    // 01/01/2024 = 45292 no Excel.
    assert!(xml.contains("<formula1>45292</formula1>"), "{xml}");
}

#[test]
fn aba_sem_validacao_nao_ganha_bloco() {
    let mut w = Workbook::new();
    w.criar_aba("Planilha1");
    let partes = montar_partes(&w);
    let xml = String::from_utf8(partes.iter().find(|(n, _)| n == "xl/worksheets/sheet1.xml").unwrap().1.clone()).unwrap();
    assert!(!xml.contains("dataValidations"));
}

/// Arquivo real do usuário (17 listas, `INDIRECT(E28)`). Só lê o original;
/// a cópia gravada vai para o diretório temporário e é apagada.
#[test]
#[ignore = "depende do arquivo do usuário em ~/Documentos/FeatherSheet"]
fn planilha_de_informacoes_v6_vai_e_volta() {
    let caminho = std::path::PathBuf::from(std::env::var("HOME").unwrap())
        .join("Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm");
    let (original, _) = feathersheet_io::xlsx_read::ler_xlsx_arquivo(&caminho).expect("ler o .xlsm");
    let bytes = gerar_xlsx(&original).expect("gravar");
    let destino = std::env::temp_dir().join(format!("validacao_v6_{}.xlsx", std::process::id()));
    std::fs::write(&destino, &bytes).unwrap();
    let relida = feathersheet_io::xlsx_read::ler_xlsx_arquivo(&destino);
    let _ = std::fs::remove_file(&destino);
    let (relida, _) = relida.expect("reler o .xlsx gravado");
    assert_eq!(original.len(), relida.len());
    let mut total = 0;
    let mut indireto = 0;
    for i in 0..original.len() {
        let a = regras(&original, i);
        let b = regras(&relida, i);
        let nome = &original.nomes()[i];
        for r in &a {
            if matches!(&r.regra, Regra::Formula { formula } if { let f = formula.to_uppercase(); f.contains("INDIRETO(") || f.contains("INDIRECT(") }) {
                indireto += 1;
            }
        }
        eprintln!("aba {nome:?}: {} regra(s) lidas, {} depois de gravar e reler", a.len(), b.len());
        assert_eq!(a, b, "aba {nome:?}");
        total += a.len();
    }
    eprintln!("total: {total} regra(s), {indireto} com INDIRETO");
    assert!(total > 0);
}

fn ida_e_volta(originais: &[Validacao]) -> Vec<Validacao> {
    let mut w = Workbook::new();
    w.criar_aba("Planilha1");
    w.aba_mut(0).unwrap().sheet.va = Some(serde_json::to_string(originais).unwrap());
    let (lida, _) = ler_xlsx_workbook(gerar_xlsx(&w).expect("gravar")).expect("ler");
    regras(&lida, 0)
}

#[test]
fn hora_caixa_email_url_e_texto_voltam_do_xlsx() {
    let r = |c: u32| vec![Rect::new(1, c, 9, c)];
    let originais = vec![
        v(r(0), Regra::Hora { op: NumeroOp::Entre, a: "08:00".into(), b: Some("18:30".into()) }, QuandoValidacao::Recusar, None),
        v(r(1), Regra::Hora { op: NumeroOp::Maior, a: "12:00:30".into(), b: None }, QuandoValidacao::Avisar, None),
        v(r(2), Regra::Caixa { marcado: "Sim".into(), desmarcado: "Não".into() }, QuandoValidacao::Recusar, None),
        v(r(3), Regra::Caixa { marcado: "VERDADEIRO".into(), desmarcado: "FALSO".into() }, QuandoValidacao::Recusar, None),
        v(r(4), Regra::Texto { op: TextoOp::Email, valor: None }, QuandoValidacao::Recusar, None),
        v(r(5), Regra::Texto { op: TextoOp::Url, valor: None }, QuandoValidacao::Avisar, None),
        v(r(6), Regra::Texto { op: TextoOp::Contem, valor: Some("a\"b".into()) }, QuandoValidacao::Recusar, None),
        v(r(7), Regra::Texto { op: TextoOp::Comeca, valor: Some("ID-".into()) }, QuandoValidacao::Recusar, None),
    ];
    assert_eq!(ida_e_volta(&originais), originais);
}

#[test]
fn mensagem_de_entrada_alerta_e_estilos_voltam_do_xlsx() {
    let mut a = v(vec![Rect::new(0, 0, 4, 0)], Regra::Lista { valores: vec!["x".into()] }, QuandoValidacao::Informar, Some("Só informa"));
    a.mensagens = MensagensValidacao {
        entrada_titulo: Some("Dica & <título>".into()),
        entrada_texto: Some("Escolha \"x\"".into()),
        erro_titulo: Some("Aviso".into()),
    };
    let mut b = v(vec![Rect::new(0, 1, 4, 1)], Regra::Numero { op: NumeroOp::Maior, a: 1.0, b: None, inteiro: false }, QuandoValidacao::Avisar, None);
    b.mensagens.erro_titulo = Some("Cuidado".into());
    let c = v(vec![Rect::new(0, 2, 4, 2)], Regra::Numero { op: NumeroOp::Maior, a: 1.0, b: None, inteiro: false }, QuandoValidacao::Recusar, None);
    let originais = vec![a, b, c];
    assert_eq!(ida_e_volta(&originais), originais);

    let mut w = Workbook::new();
    w.criar_aba("Planilha1");
    w.escrever_celula(0, 0, 0, "x".to_string());
    w.aba_mut(0).unwrap().sheet.va = Some(serde_json::to_string(&originais).unwrap());
    let partes = montar_partes(&w);
    let xml = String::from_utf8(partes.iter().find(|(n, _)| n == "xl/worksheets/sheet1.xml").unwrap().1.clone()).unwrap();
    for esperado in ["errorStyle=\"information\"", "errorStyle=\"warning\"", "errorStyle=\"stop\"", "showInputMessage=\"1\"",
        "promptTitle=\"Dica &amp; &lt;título&gt;\"", "errorTitle=\"Aviso\""] {
        assert!(xml.contains(esperado), "faltou {esperado}: {xml}");
    }
}
