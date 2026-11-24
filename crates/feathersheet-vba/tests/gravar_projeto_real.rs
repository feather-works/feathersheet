//! O gravador do projeto VBA contra um `.xlsm` gravado pelo Excel (a planilha real do usuário, só leitura).
use std::io::{Cursor, Read};

fn projeto_real() -> Option<Vec<u8>> {
    let caminho = format!("{}/Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm", std::env::var("HOME").ok()?);
    let bytes = std::fs::read(caminho).ok()?;
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut bin = Vec::new();
    zip.by_name("xl/vbaProject.bin").ok()?.read_to_end(&mut bin).ok()?;
    Some(bin)
}

/// A cifra do `CMG`/`DPB`/`GC` decifra o que o Excel gravou em estados que fazem sentido.
#[test]
#[ignore = "depende da planilha do usuário"]
fn cifra_decifra_o_que_o_excel_gravou() {
    let bin = projeto_real().expect("planilha do usuário");
    let mut cfb = cfb::CompoundFile::open(Cursor::new(bin)).unwrap();
    let mut texto = Vec::new();
    cfb.open_stream("/PROJECT").unwrap().read_to_end(&mut texto).unwrap();
    let texto = String::from_utf8_lossy(&texto).into_owned();
    let campo = |k: &str| texto.lines().find_map(|l| l.strip_prefix(&format!("{k}=\""))).map(|v| v.trim_end_matches('"').to_string()).unwrap();
    let cmg = feathersheet_vba::ovba_projeto::decifrar(&campo("CMG")).expect("CMG decifra");
    let gc = feathersheet_vba::ovba_projeto::decifrar(&campo("GC")).expect("GC decifra");
    let dpb = feathersheet_vba::ovba_projeto::decifrar(&campo("DPB")).expect("DPB decifra");
    assert_eq!(cmg.len(), 4, "ProjectProtectionState tem 4 bytes: {cmg:?}");
    assert!(!gc.is_empty() && (gc[0] == 0xFF || gc[0] == 0x00), "visibilidade 0xFF ou 0x00: {gc:?}");
    assert!(dpb.len() == 1 || dpb.len() == 29, "senha: 1 byte (sem) ou 29 (hash): {}", dpb.len());
}

/// Trocar um módulo do projeto real deixa os outros (e os formulários) lidos iguais.
#[test]
#[ignore = "depende da planilha do usuário"]
fn alterar_o_projeto_real_preserva_o_resto() {
    let bin = projeto_real().expect("planilha do usuário");
    let antes = feathersheet_vba::ovba::ler_projeto(&bin).unwrap();
    let padrao = antes.iter().find(|m| m.tipo == feathersheet_vba::TipoDeModulo::Padrao).expect("um módulo padrão");
    let novo = feathersheet_vba::ovba_projeto::alterar_projeto(&bin, &[
        feathersheet_vba::ovba_projeto::ModuloNovo { nome: padrao.nome.clone(), codigo: "Sub Trocado()\n    MsgBox \"Preço R$ 1.234,50\"\nEnd Sub".into() },
        feathersheet_vba::ovba_projeto::ModuloNovo { nome: "FeatherNovo".into(), codigo: "Sub Novo()\nEnd Sub".into() },
    ]).unwrap();
    let depois = feathersheet_vba::ovba::ler_projeto(&novo).unwrap();
    assert_eq!(depois.len(), antes.len() + 1);
    for (a, d) in antes.iter().zip(&depois) {
        assert_eq!(a.nome, d.nome);
        if a.nome == padrao.nome {
            assert!(d.codigo.contains("Preço R$ 1.234,50") && d.codigo.starts_with("Attribute VB_Name"), "{}", d.codigo);
        } else {
            assert_eq!(a.codigo, d.codigo, "{} mudou", a.nome);
        }
    }
    assert_eq!(depois.last().unwrap().nome, "FeatherNovo");
    let forms_antes = feathersheet_vba::formulario::ler_do_projeto(&bin, &antes).len();
    assert_eq!(feathersheet_vba::formulario::ler_do_projeto(&novo, &depois).len(), forms_antes, "formulários intactos");
}
