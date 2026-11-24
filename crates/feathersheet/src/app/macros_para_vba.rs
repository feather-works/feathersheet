//! Salvar em `.xlsm` leva as macros JavaScript criadas ou editadas aqui para o projeto VBA (o caminho de volta):
//! a editada volta para dentro do módulo VBA de onde veio (só o procedimento dela é trocado); a criada aqui vira
//! um módulo novo. Pasta sem VBA ganha um projeto novo. Eventos (`Eventos/…`) ainda ficam de fora, com aviso.
//! O tradutor é o `feathersheet_vba::js_para_vba`; o gravador, o `feathersheet_vba::ovba_projeto`.
use super::*;
use feathersheet_vba::ovba_projeto::ModuloNovo;

/// O módulo VBA de onde a macro veio (`// Convertida do VBA de X (módulo Modulo1).`).
fn modulo_de_origem(codigo: &str) -> Option<String> {
    let linha = codigo.lines().next()?;
    let (_, depois) = linha.split_once("(módulo ")?;
    Some(depois.trim_end_matches(").").trim_end_matches(')').to_string())
}

/// Um nome que o VBA aceita para módulo ou procedimento (até 31 letras, sem `/` nem espaço).
fn nome_vba(n: &str) -> String {
    let base = n.rsplit('/').next().unwrap_or(n);
    let mut s: String = base.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if !s.starts_with(|c: char| c.is_ascii_alphabetic()) { s = format!("M{s}"); }
    s.chars().take(31).collect()
}

/// Os procedimentos (`Sub`/`Function`) de um código VBA: (nome, texto do bloco com o `End`).
fn procedimentos(vba: &str) -> Vec<(String, String)> {
    let mut saida = Vec::new();
    let mut atual: Option<(String, String)> = None;
    for linha in vba.split_inclusive('\n') {
        let t = linha.trim_start();
        let sem_escopo = t.strip_prefix("Public ").or_else(|| t.strip_prefix("Private ")).unwrap_or(t);
        let abre = ["Sub ", "Function "].iter().find_map(|k| sem_escopo.strip_prefix(k))
            .map(|r| r.split(|c: char| c == '(' || c.is_whitespace()).next().unwrap_or("").to_string());
        match (&mut atual, abre) {
            (None, Some(nome)) => atual = Some((nome, linha.to_string())),
            (Some((_, texto)), _) => {
                texto.push_str(linha);
                if t.starts_with("End Sub") || t.starts_with("End Function") {
                    saida.push(atual.take().expect("aberto"));
                }
            }
            _ => {}
        }
    }
    saida
}

/// O código do módulo com o procedimento `nome` trocado por `novo` (ou acrescentado no fim, se não havia).
fn trocar_procedimento(fonte: &str, nome: &str, novo: &str) -> String {
    match procedimentos(fonte).into_iter().find(|(n, _)| n.eq_ignore_ascii_case(nome)) {
        Some((_, antigo)) => fonte.replacen(&antigo, novo, 1),
        None => format!("{}\r\n{novo}", fonte.trim_end()),
    }
}

/// O código do módulo sem as linhas `Attribute` do topo (o gravador põe de volta as do módulo original).
fn sem_atributos(codigo: &str) -> String {
    codigo.lines().skip_while(|l| l.starts_with("Attribute ")).collect::<Vec<_>>().join("\r\n")
}

impl App {
    /// Antes de gravar um `.xlsm`: as macros JS que não estão no VBA vão para ele. Devolve o resumo para o status
    /// (`None` = não havia nada a levar). As pendências da tradução vão para o monitor (F12).
    pub(super) fn levar_macros_para_o_vba(&mut self) -> Option<String> {
        let fora: Vec<feather_macros::Macro> = self.macro_catalogo.macros.iter()
            .filter(|m| !self.macros_do_vba.contains(&importar_vba::marca_do_codigo(m))).cloned().collect();
        if fora.is_empty() { return None; }
        let formula = |f: &str| format!("={}", feathersheet_io::xlsx_write::formula_to_excel(f));
        let originais = self.workbook.vba.as_ref()
            .and_then(|v| feathersheet_vba::ovba::ler_projeto(&v.binario).ok()).unwrap_or_default();
        let mut modulos: Vec<ModuloNovo> = Vec::new();
        let (mut levadas, mut eventos, mut pendencias) = (Vec::new(), 0, 0);
        for m in &fora {
            if m.name.starts_with(macros::organizacao::PASTA_EVENTOS) { eventos += 1; continue; }
            // O módulo de pendências da conversão é só comentário: não volta.
            if m.code.starts_with("// O que do VBA de") { continue; }
            let nome = nome_vba(&m.name);
            let t = feathersheet_vba::js_para_vba::traduzir(&m.code, &nome, &formula);
            for p in &t.pendencias {
                self.monitor.registrar(feather_ui::monitor::Nivel::Aviso, tr!("Macros"), &trf!("[VBA] {}: {p}", m.name));
            }
            pendencias += t.pendencias.len();
            let origem = modulo_de_origem(&m.code)
                .and_then(|o| originais.iter().find(|x| x.nome.eq_ignore_ascii_case(&o)).map(|x| x.nome.clone()));
            match origem {
                // Editada: os procedimentos dela trocam os de mesmo nome dentro do módulo de onde veio.
                Some(o) => {
                    let pos = match modulos.iter().position(|x| x.nome == o) {
                        Some(i) => i,
                        None => {
                            let codigo = originais.iter().find(|x| x.nome == o).map(|x| sem_atributos(&x.codigo)).unwrap_or_default();
                            modulos.push(ModuloNovo { nome: o.clone(), codigo });
                            modulos.len() - 1
                        }
                    };
                    for (proc, bloco) in procedimentos(&t.vba) {
                        modulos[pos].codigo = trocar_procedimento(&modulos[pos].codigo, &proc, &bloco);
                    }
                }
                None => {
                    let mut unico = nome.clone();
                    let mut n = 2;
                    while originais.iter().any(|x| x.nome.eq_ignore_ascii_case(&unico)) || modulos.iter().any(|x| x.nome.eq_ignore_ascii_case(&unico)) {
                        unico = format!("{}_{n}", nome.chars().take(28).collect::<String>());
                        n += 1;
                    }
                    modulos.push(ModuloNovo { nome: unico, codigo: t.vba });
                }
            }
            levadas.push(m);
        }
        if modulos.is_empty() {
            return (eventos > 0).then(|| trf!("{eventos} evento(s) editado(s) aqui ainda não vão para o VBA do .xlsm."));
        }
        let gravado = match self.workbook.vba.as_ref() {
            Some(v) => feathersheet_vba::ovba_projeto::alterar_projeto(&v.binario, &modulos).map(|b| (b, v.nome_pasta.clone(), v.nomes_abas.clone())),
            None => {
                let abas: Vec<(u32, String)> = (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i))
                    .enumerate().map(|(i, a)| (a.id, format!("Planilha{}", i + 1))).collect();
                let nomes: Vec<String> = abas.iter().map(|(_, n)| n.clone()).collect();
                feathersheet_vba::ovba_projeto::projeto_novo("EstaPasta_de_trabalho", &nomes, &modulos)
                    .map(|b| (b, "EstaPasta_de_trabalho".to_string(), abas))
            }
        };
        match gravado {
            Ok((binario, nome_pasta, nomes_abas)) => {
                self.workbook.vba = Some(feathersheet_core::planilha::workbook::ProjetoVba { binario, nome_pasta, nomes_abas });
                // Agora estão no VBA: o próximo salvar não as conta como de fora.
                self.macros_do_vba.extend(levadas.iter().map(|m| importar_vba::marca_do_codigo(m)));
                let mut s = trf!("{} macro(s) JavaScript foram para o VBA do .xlsm (o Excel recompila ao abrir).", levadas.len());
                if pendencias > 0 { s.push(' '); s.push_str(&trf!("{pendencias} trecho(s) não converteram e ficaram comentados: veja o monitor (F12).")); }
                if eventos > 0 { s.push(' '); s.push_str(&trf!("{eventos} evento(s) editado(s) aqui ainda não vão para o VBA do .xlsm.")); }
                Some(s)
            }
            Err(e) => Some(trf!("As macros JavaScript não foram para o VBA: {e}")),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn procedimento_trocado_no_lugar_e_o_resto_fica() {
        let fonte = "Option Explicit\r\nSub A()\r\n    x = 1\r\nEnd Sub\r\nPrivate Function B(y)\r\n    B = y\r\nEnd Function\r\n";
        let novo = trocar_procedimento(fonte, "a", "Sub A()\r\n    x = 2\r\nEnd Sub\r\n");
        assert_eq!(novo, "Option Explicit\r\nSub A()\r\n    x = 2\r\nEnd Sub\r\nPrivate Function B(y)\r\n    B = y\r\nEnd Function\r\n");
        assert!(trocar_procedimento(fonte, "C", "Sub C()\r\nEnd Sub\r\n").ends_with("End Function\r\nSub C()\r\nEnd Sub\r\n"));
        assert_eq!(modulo_de_origem("// Convertida do VBA de Vendas.xlsm (módulo Modulo1).\nfunction A(){}").as_deref(), Some("Modulo1"));
        assert_eq!(nome_vba("Relatórios/Fechar mês"), "Fechar_m_s");
    }

    /// Pasta com VBA convertido: editar uma macro e criar outra, salvar em .xlsm; o projeto relido tem o
    /// procedimento novo no módulo de origem (o outro procedimento intacto) e o módulo novo.
    #[test]
    fn salvar_xlsm_leva_a_editada_e_a_nova_para_o_vba() {
        let bin = feathersheet_vba::ovba_projeto::projeto_novo("EstaPasta_de_trabalho", &["Planilha1".into()], &[ModuloNovo {
            nome: "Modulo1".into(), codigo: "Sub Ola()\r\n    Range(\"A1\").Value = 1\r\nEnd Sub\r\nSub Fica()\r\n    MsgBox \"fica\"\r\nEnd Sub".into() }]).unwrap();
        let mut app = App::default();
        let id = app.workbook.aba(0).unwrap().id;
        app.workbook.vba = Some(feathersheet_core::planilha::workbook::ProjetoVba { binario: bin, nome_pasta: "EstaPasta_de_trabalho".into(), nomes_abas: vec![(id, "Planilha1".into())] });
        let modulos = feathersheet_vba::ovba::ler_projeto(&app.workbook.vba.as_ref().unwrap().binario).unwrap();
        let c = feathersheet_vba::converter(&feathersheet_vba::ProjetoVba { modulos, ..Default::default() }, "Vendas.xlsm");
        app.instalar_vba("Vendas.xlsm", &c, false);
        let mut cat = app.macro_catalogo.clone();
        let ola = cat.macros.iter_mut().find(|m| m.code.contains("function Ola")).unwrap();
        ola.code = ola.code.replace("sheet.set(\"A1\", 1)", "sheet.set(\"A1\", 1234.5)");
        cat.macros.push(feather_macros::Macro { name: "Relatórios/Total".into(), code: "sheet.set(\"B1\", sheet.get(\"A1\") * 2);".into(), auto: false });
        assert!(app.persistir_modulos(cat, "teste"));
        assert_eq!(app.macros_fora_do_excel(), 2);
        let resumo = app.levar_macros_para_o_vba().unwrap();
        assert!(resumo.contains("2 macro(s) JavaScript foram para o VBA"), "{resumo}");
        assert_eq!(app.macros_fora_do_excel(), 0, "agora estão no VBA");
        let lidos = feathersheet_vba::ovba::ler_projeto(&app.workbook.vba.as_ref().unwrap().binario).unwrap();
        let m1 = lidos.iter().find(|m| m.nome == "Modulo1").unwrap();
        assert!(m1.codigo.contains("Range(\"A1\").Value = 1234.5") && m1.codigo.contains("MsgBox \"fica\""), "{}", m1.codigo);
        let novo = lidos.iter().find(|m| m.nome == "Total").expect("módulo novo");
        assert!(novo.codigo.contains("Sub Total()") && novo.codigo.contains("Range(\"B1\").Value = Range(\"A1\").Value * 2"), "{}", novo.codigo);
    }

    /// A planilha real do usuário (só leitura): abrir, editar uma macro convertida, salvar em .xlsm numa pasta
    /// temporária e reler; o procedimento mudou e todos os outros módulos e formulários ficaram iguais.
    #[test]
    #[ignore = "depende da planilha do usuário"]
    fn planilha_real_editada_vai_e_volta_pelo_xlsm() {
        let origem = std::path::PathBuf::from(format!("{}/Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm", std::env::var("HOME").unwrap()));
        let bytes = std::fs::read(&origem).unwrap();
        let antes = feathersheet_vba::xlsm::ler(&bytes).unwrap();
        let mut app = App::default();
        app.carregar(origem.clone(), bytes).unwrap();
        if app.macro_catalogo.macros.is_empty() { app.ler_vba_do_xlsm(&origem); }
        let mut cat = app.macro_catalogo.clone();
        let alvo = cat.macros.iter_mut().find(|m| m.code.starts_with("// Convertida do VBA") && m.code.contains("function ")).expect("uma macro convertida");
        let (nome_js, modulo) = (alvo.name.clone(), modulo_de_origem(&alvo.code).unwrap());
        alvo.code.push_str("\nfunction FeatherTeste() { sheet.set(\"Z1\", 1234.5); }\n");
        assert!(app.persistir_modulos(cat, "teste"));
        let dir = std::env::temp_dir().join(format!("fs-xlsm-real-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let destino = dir.join("copia.xlsm");
        let _ = app.update(Msg::DestinoSalvar(Some(destino.clone())));
        app.concluir_gravacao_teste(destino.clone(), false);
        assert!(app.status.contains("foram para o VBA"), "{}", app.status);
        let depois = feathersheet_vba::xlsm::ler(&std::fs::read(&destino).unwrap()).unwrap();
        assert_eq!(depois.formularios.len(), antes.formularios.len(), "formulários");
        for a in &antes.modulos {
            let d = depois.modulos.iter().find(|m| m.nome == a.nome).unwrap_or_else(|| panic!("sumiu {}", a.nome));
            if a.nome == modulo {
                assert!(d.codigo.contains("Sub FeatherTeste()") && d.codigo.contains("1234.5"), "{nome_js}: {}", d.codigo);
            } else {
                assert_eq!(a.codigo, d.codigo, "{} mudou", a.nome);
            }
        }
        assert!(feathersheet_io::xlsx_read::ler_xlsx_arquivo(&destino).is_ok(), "o .xlsm reabre");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
