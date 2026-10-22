//! Organização dos módulos no editor de macros: código de eventos de cada
//! planilha, pastas e renomear.
//!
//! Tudo pelo nome do módulo, sem mudar o formato do arquivo:
//! - `Eventos/<aba>/<evento>` é o código de evento da aba (`aoMudar`,
//!   `aoAtivar`); ao abrir o arquivo a lista de eventos é refeita a partir
//!   desses nomes, então eles passam a valer depois de salvar e reabrir;
//! - `pasta/sub/nome` guarda o módulo numa pasta (o Explorador agrupa).
use super::*;

/// Pasta reservada do código de eventos das planilhas.
pub(in crate::app) const PASTA_EVENTOS: &str = "Eventos/";

/// Os eventos de planilha que o motor dispara (eventos_vba.rs): chave e rótulo.
pub(in crate::app) const EVENTOS_PLANILHA: [(&str, &str); 2] = [
    ("aoMudar", "Ao mudar células"),
    ("aoAtivar", "Ao ativar a aba"),
];

pub(in crate::app) fn nome_evento(aba: &str, evento: &str) -> String {
    format!("{PASTA_EVENTOS}{aba}/{evento}")
}

/// `Eventos/<aba>/<evento>` → (aba, evento), se o evento é conhecido.
pub(in crate::app) fn evento_do_nome(nome: &str) -> Option<(&str, &str)> {
    let resto = nome.strip_prefix(PASTA_EVENTOS)?;
    let (aba, evento) = resto.rsplit_once('/')?;
    EVENTOS_PLANILHA.iter().any(|(e, _)| *e == evento).then_some((aba, evento))
        .filter(|(aba, _)| !aba.is_empty())
}

/// Código inicial de um evento ainda vazio (só comentários: não faz nada).
fn modelo_evento(aba: &str, evento: &str) -> String {
    // O "//" fica fora do texto traduzido (no dicionário, "//" é comentário).
    match evento {
        "aoAtivar" => format!("// {}\n", trf!("Roda quando alguém troca para a aba {}.", aba)),
        _ => format!("// {}\n// {}\n",
            trf!("Roda depois de cada mudança feita à mão nas células da aba {}.", aba),
            tr!("valores.intervalo traz o endereço mudado (ex.: \"B2:C4\").")),
    }
}

/// O código só tem comentários e espaços (o modelo, por exemplo)?
fn so_comentarios(codigo: &str) -> bool {
    codigo.lines().all(|l| { let l = l.trim(); l.is_empty() || l.starts_with("//") })
}

/// Nome de módulo válido: 1 a 128 caracteres, partes da pasta não vazias.
fn nome_valido(nome: &str) -> Result<(), String> {
    if nome.is_empty() || nome.chars().count() > 128 {
        return Err(tr!("O nome deve ter de 1 a 128 caracteres.").into());
    }
    if nome.split('/').any(|p| p.trim().is_empty()) {
        return Err(tr!("Pasta ou nome vazio entre as barras.").into());
    }
    Ok(())
}

impl App {
    /// Refaz a lista de eventos a partir dos módulos `Eventos/…`, mantendo
    /// os do VBA importado nesta sessão (que têm outros nomes).
    pub(in crate::app) fn sincronizar_eventos(&mut self) {
        self.eventos_vba.retain(|e| !e.modulo.starts_with(PASTA_EVENTOS));
        for m in &self.macro_catalogo.macros {
            if let Some((aba, evento)) = evento_do_nome(&m.name) {
                if so_comentarios(&m.code) { continue; }
                self.eventos_vba.push(crate::app::eventos_vba::EventoVba {
                    aba: aba.to_owned(), evento: evento.to_owned(), modulo: m.name.clone(),
                });
            }
        }
    }

    /// Clicar numa planilha no Explorador: abre o código do evento escolhido
    /// (o guardado ou o modelo, se ainda não existe).
    pub(in crate::app) fn abrir_evento_planilha(&mut self, aba: &str) {
        let evento = self.macro_visual.evento_planilha;
        let nome = nome_evento(aba, evento);
        let codigo = self.macro_catalogo.obter(&nome).map(|m| m.code.clone())
            .unwrap_or_else(|| modelo_evento(aba, evento));
        self.macro_nome = nome;
        self.macro_codigo = pelicano::Content::with_text(&codigo);
        self.macro_visual.documento = format!("planilha:{aba}");
    }

    /// Guardar o evento: só com código de verdade; vazio (ou só o modelo)
    /// remove o módulo, e o evento deixa de rodar.
    pub(in crate::app) fn guardar_evento_planilha(&mut self) {
        let nome = self.macro_nome.clone();
        let codigo = self.macro_codigo.text();
        let mut c = self.macro_catalogo.clone();
        c.macros.retain(|m| !m.name.eq_ignore_ascii_case(&nome));
        let status = if so_comentarios(&codigo) {
            trf!("Evento {nome} sem código: não roda")
        } else {
            c.macros.push(feather_macros::Macro { name: nome.clone(), code: codigo, auto: false });
            trf!("Evento {nome} salvo")
        };
        self.persistir_modulos(c, &status);
    }

    /// Nova pasta: cria `PastaN/Modulo1` e já abre o nome para editar.
    pub(in crate::app) fn nova_pasta_macro(&mut self) {
        let mut n = 1usize;
        let pasta = loop {
            let p = format!("Pasta{n}/");
            if !self.macro_catalogo.macros.iter().any(|m| m.name.starts_with(&p)) { break p; }
            n += 1;
        };
        let nome = format!("{pasta}Modulo1");
        let codigo = format!("// {nome} · JavaScript do FeatherSheet\n");
        let mut c = self.macro_catalogo.clone();
        c.macros.push(feather_macros::Macro { name: nome.clone(), code: codigo.clone(), auto: false });
        if self.persistir_modulos(c, &trf!("Pasta {} criada", pasta.trim_end_matches('/'))) {
            self.macro_nome = nome.clone();
            self.macro_codigo = pelicano::Content::with_text(&codigo);
            self.macro_visual.documento = format!("modulo:{nome}");
            self.macro_visual.renomeando = Some(nome);
        }
    }

    /// Renomeia (ou move para outra pasta) o módulo aberto; os botões de
    /// macro da planilha que o chamavam passam a chamar o nome novo.
    pub(in crate::app) fn renomear_modulo(&mut self, novo: &str) -> Result<(), String> {
        let antigo = self.macro_nome.clone();
        let novo = novo.trim().split('/').map(str::trim).collect::<Vec<_>>().join("/");
        nome_valido(&novo)?;
        if novo.starts_with(PASTA_EVENTOS) {
            return Err(tr!("A pasta Eventos é reservada ao código das planilhas.").into());
        }
        if novo == antigo { return Ok(()); }
        if !novo.eq_ignore_ascii_case(&antigo) && self.macro_catalogo.obter(&novo).is_some() {
            return Err(trf!("Já existe um módulo chamado {novo}."));
        }
        let mut c = self.macro_catalogo.clone();
        let Some(m) = c.macros.iter_mut().find(|m| m.name.eq_ignore_ascii_case(&antigo)) else {
            return Err(tr!("Guarde o módulo antes de renomear.").into());
        };
        m.name = novo.clone();
        for i in 0..self.workbook.len() {
            let Some(aba) = self.workbook.aba_mut(i) else { continue };
            let Some(bt) = aba.sheet.bt.as_deref() else { continue };
            let Ok(mut itens) = serde_json::from_str::<Vec<serde_json::Value>>(bt) else { continue };
            let mut mudou = false;
            for item in &mut itens {
                if item["macroName"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(&antigo)) {
                    item["macroName"] = serde_json::json!(novo);
                    mudou = true;
                }
            }
            if mudou { aba.sheet.bt = serde_json::to_string(&itens).ok(); }
        }
        if self.persistir_modulos(c, &trf!("Módulo {antigo} agora é {novo}")) {
            self.macro_nome = novo.clone();
            self.macro_visual.documento = format!("modulo:{novo}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn app_com(modulos: &[(&str, &str)]) -> App {
        let mut app = App::default();
        app.macro_catalogo = feather_macros::Catalogo {
            macros: modulos.iter().map(|(n, c)| feather_macros::Macro {
                name: (*n).into(), code: (*c).into(), auto: false }).collect(),
        };
        app
    }

    #[test]
    fn nome_de_evento_vai_e_volta() {
        assert_eq!(nome_evento("Resumo", "aoMudar"), "Eventos/Resumo/aoMudar");
        assert_eq!(evento_do_nome("Eventos/Resumo/aoMudar"), Some(("Resumo", "aoMudar")));
        assert_eq!(evento_do_nome("Eventos/Resumo/outro"), None);
        assert_eq!(evento_do_nome("Eventos//aoMudar"), None);
        assert_eq!(evento_do_nome("pasta/aoMudar"), None);
    }

    #[test]
    fn eventos_sao_refeitos_pelos_nomes_e_o_modelo_vazio_nao_roda() {
        let mut app = app_com(&[
            ("Eventos/Planilha1/aoMudar", "sheet.range('A1').value = 1;"),
            ("Eventos/Planilha1/aoAtivar", "// só comentário\n"),
            ("Modulo1", "log(1)"),
        ]);
        app.sincronizar_eventos();
        assert_eq!(app.eventos_vba.len(), 1);
        assert_eq!(app.eventos_vba[0].evento, "aoMudar");
        assert_eq!(app.eventos_vba[0].modulo, "Eventos/Planilha1/aoMudar");
    }

    #[test]
    fn clicar_na_planilha_abre_o_modelo_e_guardar_liga_o_evento() {
        let mut app = app_com(&[]);
        app.abrir_evento_planilha("Planilha1");
        assert_eq!(app.macro_nome, "Eventos/Planilha1/aoMudar");
        assert!(app.macro_codigo.text().starts_with("//"));
        // Guardar só o modelo não cria nada.
        app.guardar_evento_planilha();
        assert!(app.macro_catalogo.macros.is_empty());
        app.macro_codigo = pelicano::Content::with_text("log(valores.intervalo);");
        app.guardar_evento_planilha();
        assert!(app.macro_catalogo.obter("Eventos/Planilha1/aoMudar").is_some());
        assert_eq!(app.eventos_vba.len(), 1);
        assert_eq!(app.macro_visual.documento, "planilha:Planilha1");
    }

    #[test]
    fn renomear_move_para_pasta_e_atualiza_os_botoes() {
        let mut app = app_com(&[("macro_1", "log(1)"), ("macro_2", "log(2)")]);
        app.workbook.aba_mut(0).unwrap().sheet.bt =
            Some(r#"[{"macroName":"macro_1","label":"Rodar"}]"#.into());
        app.macro_nome = "macro_1".into();
        app.renomear_modulo(" macros_auxiliares / macro_1 ").unwrap();
        assert!(app.macro_catalogo.obter("macros_auxiliares/macro_1").is_some());
        assert!(app.workbook.aba(0).unwrap().sheet.bt.as_deref().unwrap()
            .contains("macros_auxiliares/macro_1"));
        assert!(app.renomear_modulo("macro_2").is_err(), "nome repetido");
        assert!(app.renomear_modulo("a//b").is_err());
        assert!(app.renomear_modulo("Eventos/x/aoMudar").is_err());
    }

    #[test]
    fn nova_pasta_usa_nome_livre_e_abre_o_renomear() {
        let mut app = app_com(&[("Pasta1/Modulo1", "")]);
        app.nova_pasta_macro();
        assert!(app.macro_catalogo.obter("Pasta2/Modulo1").is_some());
        assert_eq!(app.macro_visual.renomeando.as_deref(), Some("Pasta2/Modulo1"));
    }
}
