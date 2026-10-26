//! Edição e abertura de hiperlinks por célula no FeatherSheet Rust/Iced.
//! O endereço é um campo de estilo, não parte do texto da célula.
use super::*;
use std::process::Command;

#[derive(Debug, Clone)]
pub(super) struct Estado {
    pub folha: usize,
    pub linha: u32,
    pub coluna: u32,
    pub endereco: String,
    pub texto: String,
    pub formula: bool,
    pub erro: Option<String>,
}

/// Mesma política de src/ui/grade/link.ts: esquema web/mailto ou www.,
/// sem espaços, tags nem esquemas executáveis. Não faz chamadas de rede.
pub(crate) fn normalizar_link(valor: &str) -> Option<String> {
    let valor = valor.trim();
    if !(8..=2048).contains(&valor.len()) || valor.chars().any(|c|
        c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"' | '\\')
    ) {
        return None;
    }
    // Caminho quente do Canvas: não alocar lowercase para cada célula visível.
    let prefixo = |p: &str| valor.get(..p.len())
        .is_some_and(|inicio| inicio.eq_ignore_ascii_case(p));
    if prefixo("https://") || prefixo("http://") || prefixo("mailto:") {
        let apos = valor.split_once(':')?.1;
        if apos.is_empty() || (apos.starts_with("//") && apos.len() <= 2) {
            return None;
        }
        return Some(valor.to_owned());
    }
    if prefixo("www.") {
        let resto = &valor[4..];
        let (dominio, _) = resto.split_once('.').unwrap_or(("", ""));
        if !dominio.is_empty() && resto.split_once('.').is_some_and(|(_, t)| !t.is_empty()) {
            return Some(format!("https://{valor}"));
        }
    }
    None
}

fn abrir_no_sistema(url: &str) -> Result<(), String> {
    // Sem shell ou expansão de argumentos; o destino já foi validado.
    #[cfg(target_os = "windows")]
    let resultado = Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let resultado = Command::new("open").arg(url).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let resultado = Command::new("xdg-open").arg(url).spawn();
    resultado.map(|_| ()).map_err(|e| trf!("Não foi possível abrir o navegador: {e}"))
}

impl App {
    /// Endereço explícito do .xlsx/estilo tem preferência; texto puro
    /// também vira um link reconhecido sem alterar o arquivo ao carregar CSV.
    pub(super) fn link_da_celula(&self, folha: usize, linha: u32, coluna: u32) -> Option<String> {
        let aba = self.workbook.aba(folha)?;
        if let Some(link) = aba.sheet.estilo_resolvido(linha, coluna)
            .and_then(|estilo| estilo.link)
            .and_then(|s| normalizar_link(&s))
        {
            return Some(link);
        }
        normalizar_link(&aba.sheet.get_raw(linha, coluna))
    }

    pub(super) fn editar_link(&mut self) -> Task<Msg> {
        if !self.confirmar() { return Task::none(); }
        let Some(aba) = self.workbook.aba(self.folha) else { return Task::none(); };
        let raw = aba.sheet.get_raw(self.linha, self.coluna);
        let formula = raw.starts_with('=');
        let endereco = self.link_da_celula(self.folha, self.linha, self.coluna)
            .unwrap_or_default();
        self.link_modal = Some(Estado {
            folha: self.folha, linha: self.linha, coluna: self.coluna,
            endereco, texto: raw, formula, erro: None,
        });
        self.menu_aberto = false;
        self.contexto = None;
        Task::none()
    }

    pub(super) fn salvar_link(&mut self) -> Task<Msg> {
        let Some(estado) = self.link_modal.as_ref().cloned() else { return Task::none(); };
        let link = if estado.endereco.trim().is_empty() {
            None
        } else {
            match normalizar_link(&estado.endereco) {
                Some(endereco) => Some(endereco),
                None => {
                    if let Some(ref mut modal) = self.link_modal {
                        modal.erro = Some(tr!("Use http://, https://, mailto: ou www. (sem espaços).").into());
                    }
                    return Task::none();
                }
            }
        };
        let Some(aba) = self.workbook.aba(estado.folha) else {
            self.link_modal = None;
            return Task::none();
        };
        let anterior = aba.sheet.get_raw(estado.linha, estado.coluna);
        let estilo_link = aba.sheet.estilo_resolvido(estado.linha, estado.coluna)
            .and_then(|s| s.link);
        let novo = if estado.formula { anterior.clone() }
            else if estado.texto.trim().is_empty() {
                link.clone().unwrap_or_default()
            } else {
                estado.texto.clone()
            };
        if novo != anterior || estilo_link != link {
            let area = Rect::cell(estado.linha, estado.coluna);
            self.workbook.editar(estado.folha, |sheet| {
                // Única transação de desfazer: texto e link mudam juntos.
                sheet.abrir_transacao();
                if novo != anterior { sheet.set_cell(estado.linha, estado.coluna, &novo); }
                let mut patch = EstiloParcial::new();
                patch.insert("link".into(), link.clone().map_or(Campo::Nulo, Campo::Texto));
                sheet.set_style(&area, &patch);
                sheet.fechar_transacao();
            });
            self.alterado = true;
            self.status = if link.is_some() { tr!("Hiperlink salvo").into() } else { tr!("Hiperlink removido").into() };
            if !estado.formula && novo != anterior && estado.folha == self.folha
                && (estado.linha, estado.coluna) == (self.linha, self.coluna)
            {
                self.edicao = novo;
            }
        }
        self.link_modal = None;
        Task::none()
    }

    pub(super) fn remover_link(&mut self) -> Task<Msg> {
        if !self.confirmar() { return Task::none(); }
        let Some(aba) = self.workbook.aba(self.folha) else { return Task::none(); };
        let tinha = aba.sheet.estilo_resolvido(self.linha, self.coluna)
            .and_then(|s| s.link).is_some();
        if tinha {
            let area = Rect::cell(self.linha, self.coluna);
            let mut patch = EstiloParcial::new();
            patch.insert("link".into(), Campo::Nulo);
            self.workbook.editar(self.folha, |s| s.set_style(&area, &patch));
            self.alterado = true;
            self.status = tr!("Hiperlink removido; texto preservado").into();
        } else {
            self.status = tr!("Essa célula não tem hiperlink salvo no estilo").into();
        }
        Task::none()
    }

    /// Limpar ▾ > Limpar hiperlinks: tira o link de toda a seleção e mantém
    /// texto e formatação, num só Ctrl+Z. Em áreas pequenas só toca as
    /// células que têm link; linha/coluna inteira vira estilo de eixo.
    pub(super) fn limpar_hiperlinks_selecao(&mut self) -> Task<Msg> {
        if !self.confirmar() { return Task::none(); }
        const LIMITE: u64 = 200_000;
        let areas = self.areas_selecionadas();
        let mut patch = EstiloParcial::new();
        patch.insert("link".into(), Campo::Nulo);
        let mut removidos = 0usize;
        self.workbook.editar(self.folha, |s| {
            s.abrir_transacao();
            for area in &areas {
                let r = area.normalized();
                let total = (r.r1 - r.r0 + 1) as u64 * (r.c1 - r.c0 + 1) as u64;
                if total > LIMITE {
                    s.set_style(&r, &patch);
                    removidos += 1;
                    continue;
                }
                for l in r.r0..=r.r1 {
                    for c in r.c0..=r.c1 {
                        if s.estilo_resolvido(l, c).and_then(|e| e.link).is_some() {
                            s.set_style(&Rect::cell(l, c), &patch);
                            removidos += 1;
                        }
                    }
                }
            }
            s.fechar_transacao();
        });
        if removidos > 0 {
            self.alterado = true;
            self.status = tr!("Hiperlinks removidos da seleção; texto preservado").into();
        } else {
            self.status = tr!("A seleção não tem hiperlinks").into();
        }
        Task::none()
    }

    pub(super) fn abrir_link(&mut self) -> Task<Msg> {
        if !self.confirmar() { return Task::none(); }
        let Some(link) = self.link_da_celula(self.folha, self.linha, self.coluna) else {
            self.status = tr!("A célula não contém um endereço web válido").into();
            return Task::none();
        };
        match abrir_no_sistema(&link) {
            Ok(()) => self.status = tr!("Abrindo link no navegador…").into(),
            Err(erro) => self.status = erro,
        }
        Task::none()
    }

    fn view_link(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        use super::dialogo_interno::dlg;
        let s = self.link_modal.as_ref().expect("diálogo de link aberto");
        let titulo = if s.endereco.is_empty() { tr!("Inserir link") } else { tr!("Editar link") };
        let rotulo = |t: &str| text(t.to_owned()).size(12).color(COR_TEXTO_MUTED());
        let texto_celula = if s.formula {
            text_input(tr!("A célula contém uma fórmula"), &s.texto)
        } else {
            text_input(tr!("Texto exibido (vazio: endereço)"), &s.texto).on_input(Msg::LinkTexto)
        };
        let mut corpo = pelicano::column![
            rotulo(tr!("Endereço (http, https, mailto ou www.)")),
            text_input("https://exemplo.com", &s.endereco)
                .on_input(Msg::LinkEndereco).on_submit(Msg::LinkSalvar).padding(8).size(13),
            Space::new(0.0, 2),
            rotulo(tr!("Texto da célula")),
            texto_celula.padding(8).size(13),
        ].spacing(6);
        if s.formula {
            corpo = corpo.push(dlg::aviso_vazio(tr!("Esta célula tem fórmula: só o endereço será alterado.")));
        }
        if let Some(erro) = &s.erro {
            corpo = corpo.push(row![feather_ui::icones::icone("info-circle", 15.0),
                text(erro.clone()).size(12).color(COR_ERRO())].spacing(6).align(pelicano::Align::Center));
        }
        self.dialogo(titulo, corpo)
            .icone("link")
            .subtitulo(trf!("Célula {}", cell_name(s.linha, s.coluna)))
            .largura(460.0)
            .ao_fechar(Msg::LinkFechar)
            .secundario(tr!("Cancelar"), Some(Msg::LinkFechar))
            .primario(tr!("Salvar link"), Some(Msg::LinkSalvar))
    }

    pub(super) fn sobrepor_link<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.link_modal.is_none() { return base; }
        self.view_link().sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn bloqueia_esquemas_perigosos_e_reconhece_enderecos_csv() {
        for url in ["javascript:alert(1)", "file:///etc/passwd", "http://a b.com",
            "https://a.com/<script>", "xdg-open --help", "www.", "www.a"]
        {
            assert!(normalizar_link(url).is_none(), "{url}");
        }
        assert_eq!(normalizar_link("www.example.com"), Some("https://www.example.com".into()));
        assert_eq!(normalizar_link("https://example.com"), Some("https://example.com".into()));
        assert_eq!(normalizar_link("HtTpS://Example.com"), Some("HtTpS://Example.com".into()));
        assert_eq!(normalizar_link("mailto:teste@example.com"),
            Some("mailto:teste@example.com".into()));
    }

    #[test]
    fn edita_link_com_texto_e_permite_desfazer_transacao() {
        let mut app = App::default();
        app.update(Msg::LinkEditar);
        app.update(Msg::LinkEndereco("www.exemplo.com".into()));
        app.update(Msg::LinkTexto("Exemplo".into()));
        app.update(Msg::LinkSalvar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "Exemplo");
        assert_eq!(app.link_da_celula(0, 0, 0).as_deref(), Some("https://www.exemplo.com"));
        assert!(app.alterado);
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "");
        assert!(app.workbook.aba(0).unwrap().sheet.get_style(0,0)
            .and_then(|s| s.link.as_ref()).is_none());
    }

    #[test]
    fn menu_typescript_direciona_editar_link_para_rust() {
        let app = App::default();
        let acao = menu_nativo::Acao { id: "editarLink".into(), args: vec![] };
        assert!(matches!(app.evento_menu(&acao), Some(Msg::LinkEditar)));
    }

    #[test]
    fn formula_e_preservada_e_nao_abre_endereco_invalido() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "=1+2".into());
        app.update(Msg::LinkEditar);
        app.update(Msg::LinkEndereco("javascript:alert(1)".into()));
        app.update(Msg::LinkSalvar);
        assert!(app.link_modal.as_ref().unwrap().erro.is_some());
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "=1+2");
        app.update(Msg::LinkEndereco("https://exemplo.com".into()));
        app.update(Msg::LinkSalvar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "=1+2");
        assert_eq!(app.link_da_celula(0,0,0).as_deref(), Some("https://exemplo.com"));
        app.update(Msg::LinkRemover);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "=1+2");
    }
}
