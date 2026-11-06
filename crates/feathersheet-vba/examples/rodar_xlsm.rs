//! `cargo run --release --example rodar_xlsm -- arquivo.xlsm [filtro]`: converte as macros do
//! arquivo e RODA cada uma (e cada evento) numa cópia nova da pasta lida do próprio arquivo,
//! respondendo Sim às perguntas (e o texto padrão ao InputBox). Mostra o que deu erro: é o teste que a contagem de pendências
//! não faz (código convertido que não roda).
use feathersheet_core::planilha::workbook::Workbook;
use std::path::Path;
use std::time::{Duration, Instant};

fn pasta(caminho: &Path) -> Workbook {
    feathersheet_io::xlsx_read::ler_xlsx_arquivo(caminho).expect("ler o .xlsm").0
}

fn main() {
    let mut args = std::env::args().skip(1);
    let caminho = std::path::PathBuf::from(args.next().expect("passe o .xlsm"));
    let filtro = args.next().unwrap_or_default().to_lowercase();
    let bytes = std::fs::read(&caminho).expect("ler o arquivo");
    let c = feathersheet_vba::converter_xlsm(&bytes, &caminho.file_name().unwrap().to_string_lossy()).expect("converter");
    let mut alvos: Vec<(String, Option<String>, String)> = c.macros.iter().map(|m| (m.nome.clone(), None, m.codigo.clone())).collect();
    alvos.extend(c.eventos.iter().map(|e| (format!("evento {} {}", e.aba.as_deref().unwrap_or("pasta"), e.evento), e.aba.clone(), e.codigo.clone())));
    let (mut ok, mut erros) = (0, 0);
    for (nome, aba, codigo) in alvos.into_iter().filter(|(n, ..)| n.to_lowercase().contains(&filtro)) {
        let mut wb = pasta(&caminho);
        let ativa = aba.and_then(|a| wb.nomes().iter().position(|n| *n == a)).unwrap_or(0);
        let mut respostas = Vec::new();
        let mut escolhas: Vec<u32> = Vec::new();
        let mut textos: Vec<Option<String>> = Vec::new();
        let t = Instant::now();
        let r = loop {
            let contexto = feathersheet_macros::Contexto {
                arquivo: Some(caminho.clone()), respostas: respostas.clone(), escolhas: escolhas.clone(), textos: textos.clone(),
                limites: feathersheet_macros::Limites { prazo: Duration::from_secs(20), parar: None },
                ..Default::default()
            };
            let r = feathersheet_macros::executar_com(&mut wb, ativa, &codigo, contexto);
            // Pergunta de vários botões (MsgBox com Sim/Não/Cancelar…): o botão padrão; Sim/Não: Sim.
            if let (Some(e), true) = (&r.escolha, escolhas.len() < 20) { escolhas.push(e.padrao as u32); continue; }
            // InputBox: o valor que já vem escrito.
            if let (Some(t), true) = (&r.entrada, textos.len() < 20) { textos.push(Some(t.clone())); continue; }
            if r.pergunta.is_some() && respostas.len() < 20 { respostas.push(true); continue; }
            break r;
        };
        match &r.erro {
            None => { ok += 1; println!("ok    {nome} ({} ms)", t.elapsed().as_millis()); }
            Some(e) => { erros += 1; println!("ERRO  {nome}: {}", e.lines().next().unwrap_or("")); }
        }
    }
    println!("\n{ok} rodaram, {erros} com erro");
}
