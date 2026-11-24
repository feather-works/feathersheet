//! Relatório da conversão do VBA (referência: o Verificador de Compatibilidade do Excel): o que veio do
//! `.xlsm`, o que ficou para trás e o que pode agir diferente, cada trecho com módulo, linha, motivo e o
//! VBA original. Abre sozinho depois de "Importar VBA" com pendências e pelo menu Arquivo do editor de macros.
//! Os mesmos trechos continuam, comentados, no módulo `VBA_pendencias_<arquivo>`.
use super::*;
use feathersheet_vba::gerar::Pendencia;
use pelicano::column;

/// Linhas do VBA mostradas por trecho (o bloco inteiro vai no "Copiar relatório").
const LINHAS_DO_TRECHO: usize = 4;

#[derive(Debug, Clone, Default)]
pub(super) struct RelatorioVba {
    pub arquivo: String,
    pub macros: usize,
    pub eventos: usize,
    pub formularios: usize,
    pub pendencias: Vec<Pendencia>,
    /// O módulo de pendências no catálogo, se a conversão deixou algum.
    pub modulo: Option<String>,
    pub aberto: bool,
}

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Fechar,
    AbrirModulo,
}

impl RelatorioVba {
    fn resumo(&self) -> String {
        trf!("{} macro(s), {} evento(s), {} formulário(s), {} pendência(s)", self.macros, self.eventos, self.formularios, self.pendencias.len())
    }

    /// O relatório em texto, para colar num e-mail ou num chamado.
    pub fn texto(&self) -> String {
        let mut t = format!("{} — {}\n{}\n", tr!("Relatório da conversão do VBA"), self.arquivo, self.resumo());
        for (titulo, aviso) in [(tr!("Não convertido"), false), (tr!("Pode agir diferente"), true)] {
            let itens: Vec<&Pendencia> = self.pendencias.iter().filter(|p| p.aviso == aviso).collect();
            if itens.is_empty() { continue; }
            t.push_str(&format!("\n{titulo} ({})\n", itens.len()));
            for p in itens {
                t.push_str(&format!("- {}: {}\n", trf!("{} · linha {}", p.modulo, p.linha), p.motivo));
                for l in p.vba.trim().lines() { t.push_str(&format!("      {l}\n")); }
            }
        }
        t
    }
}

impl App {
    /// Guarda o relatório da conversão que acabou de entrar no catálogo. `novos` são os módulos criados
    /// (o de pendências é o último, quando há).
    pub(super) fn guardar_relatorio_vba(&mut self, arquivo: &str, c: &feathersheet_vba::Conversao, novos: &[feather_macros::Macro], abrir: bool) {
        let modulo = (!c.pendencias.is_empty()).then(|| novos.last().map(|m| m.name.clone())).flatten();
        self.relatorio_vba = Some(RelatorioVba {
            arquivo: arquivo.to_string(),
            macros: c.macros.len(), eventos: c.eventos.len(), formularios: c.formularios.len(),
            pendencias: c.pendencias.clone(), modulo,
            aberto: abrir && !c.pendencias.is_empty(),
        });
    }

    /// O relatório mora na janela do editor de macros: pedido pela faixa da janela principal, abre o editor.
    pub(super) fn editor_para_o_relatorio_vba(&mut self) -> Task<Msg> {
        if self.relatorio_vba.as_ref().is_some_and(|r| r.aberto) && !self.janelas.aberta(Auxiliar::Macros) {
            self.alternar(Auxiliar::Macros)
        } else {
            Task::none()
        }
    }

    pub(super) fn evento_relatorio_vba(&mut self, e: Evento) -> Task<Msg> {
        match e {
            Evento::Abrir => match &mut self.relatorio_vba {
                Some(r) => { r.aberto = true; return self.editor_para_o_relatorio_vba(); }
                None => self.status = tr!("Nenhum VBA foi convertido nesta pasta. Use Arquivo → Importar VBA do Excel (.xlsm) no editor de macros.").into(),
            },
            Evento::Fechar => if let Some(r) = &mut self.relatorio_vba { r.aberto = false; },
            Evento::AbrirModulo => {
                let modulo = self.relatorio_vba.as_mut().and_then(|r| { r.aberto = false; r.modulo.clone() });
                if let Some(m) = modulo { self.abrir_modulo_macro(&m); }
            }
        }
        Task::none()
    }

    /// `FEATHERSHEET_CAPTURAR_JANELA=relatorio_vba`: um VBA de exemplo com o que fica para trás (OnTime, Shell
    /// de outro programa) e o relatório aberto no editor. Só na memória da pasta.
    pub(super) fn preparar_captura_relatorio_vba(&mut self) -> Task<Msg> {
        let c = feathersheet_vba::converter_texto(&[("Relatorios",
            "Sub Atualizar()\n    Range(\"A1\").Value = Date\n    Application.OnTime Now + TimeValue(\"00:10:00\"), \"Atualizar\"\nEnd Sub\n\
             Sub Exportar()\n    Dim w As Object\n    Set w = CreateObject(\"Outlook.Application\")\n    w.CreateItem(0).Send\nEnd Sub\n")], &[]);
        self.instalar_vba("Vendas 2026", &c, true);
        self.alternar(Auxiliar::Macros)
    }

    pub(super) fn sobrepor_relatorio_vba<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        use super::dialogo_interno::dlg;
        let Some(r) = self.relatorio_vba.as_ref().filter(|r| r.aberto) else { return base };
        let ev = |e: Evento| Msg::RelatorioVba(e);
        let mut corpo = column![text(r.resumo()).size(13).font(pelicano::Font::MONO)].spacing(12);
        if r.pendencias.is_empty() {
            corpo = corpo.push(dlg::aviso("circle-check", tr!("Tudo do VBA foi convertido.")));
        } else if let Some(m) = &r.modulo {
            corpo = corpo.push(text(trf!("Os mesmos trechos estão, comentados, no módulo {m}. Revise antes de rodar a macro."))
                .size(12).line_height(1.4).color(COR_TEXTO_MUTED()));
        }
        for (titulo, aviso, icone) in [(tr!("Não convertido"), false, "alert-triangle"), (tr!("Pode agir diferente"), true, "info-circle")] {
            let itens: Vec<&Pendencia> = r.pendencias.iter().filter(|p| p.aviso == aviso).collect();
            if itens.is_empty() { continue; }
            corpo = corpo.push(dlg::secao(&format!("{titulo} ({})", itens.len())));
            let mut lista = column(Vec::<Element<_>>::new()).spacing(10);
            for p in itens {
                let linhas: Vec<&str> = p.vba.trim().lines().collect();
                let mut vba = linhas.iter().take(LINHAS_DO_TRECHO).copied().collect::<Vec<_>>().join("\n");
                if linhas.len() > LINHAS_DO_TRECHO { vba.push_str("\n…"); }
                lista = lista.push(column![
                    row![
                        feather_ui::icones::icone_traco(icone, 14.),
                        text(trf!("{} · linha {}", p.modulo, p.linha)).size(12.5).font(pelicano::Font { weight: 600, ..Default::default() }),
                    ].spacing(6).align(pelicano::Align::Center),
                    text(p.motivo.clone()).size(12.5).line_height(1.35),
                    text(vba).size(11.5).font(pelicano::Font::MONO).color(COR_TEXTO_MUTED()),
                ].spacing(4));
            }
            corpo = corpo.push(container(lista).padding([8, 12]).width(Length::FILL).style(dlg::estilo_bloco));
        }
        let (altura, largura) = (self.macro_visual.codigo.janela.height, self.macro_visual.codigo.janela.width);
        let mut d = dlg::Dialogo::new(tr!("Relatório da conversão do VBA"), corpo)
            .topo(52.0)
            .icone("file-code")
            .subtitulo(r.arquivo.clone())
            .largura(620.0)
            .rodape_esquerda(dlg::botao_secundario(tr!("Copiar relatório"), Some(Msg::CopiarTexto(r.texto()))))
            .botao(dlg::botao_secundario(tr!("Abrir no editor"), r.modulo.as_ref().map(|_| ev(Evento::AbrirModulo))))
            .botao(dlg::botao_primario(tr!("Fechar"), Some(ev(Evento::Fechar))))
            .ao_fechar(ev(Evento::Fechar));
        if altura > 0.0 { d = d.altura_janela(altura); }
        if largura > 0.0 { d = d.largura_janela(largura); }
        d.sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Importar VBA com pendências abre o relatório; ele separa o que ficou para trás do que pode agir diferente,
    /// o texto copiado leva o trecho inteiro e "Abrir no editor" vai para o módulo de pendências.
    #[test]
    fn relatorio_da_conversao_lista_pendencias_e_abre_o_modulo() {
        let c = feathersheet_vba::converter_texto(&[("Modulo1",
            "Sub Teste()\n    Range(\"A1\").Value = 1\n    Application.OnTime Now + TimeValue(\"00:00:05\"), \"Teste\"\nEnd Sub\n")], &[]);
        assert!(!c.pendencias.is_empty(), "o OnTime fica para trás: {:?}", c.pendencias);
        let mut app = App::default();
        app.macro_catalogo = feather_macros::Catalogo::default();
        let _ = app.update(Msg::RelatorioVba(Evento::Abrir));
        assert!(app.relatorio_vba.is_none() && app.status.contains("VBA"), "sem conversão, o menu explica: {}", app.status);

        app.instalar_vba("Vendas", &c, true);
        let r = app.relatorio_vba.clone().expect("relatório guardado");
        assert!(r.aberto, "importar com pendência abre o relatório");
        let modulo = r.modulo.clone().expect("módulo de pendências");
        assert!(app.macro_catalogo.macros.iter().any(|m| m.name == modulo));
        let texto = r.texto();
        assert!(texto.contains("Vendas") && texto.contains("OnTime") && texto.contains(&format!("{} · linha", c.pendencias[0].modulo)), "{texto}");
        let _: Element<'_, Msg> = app.sobrepor_relatorio_vba(Space::new(0.0, 0.0).into());

        let _ = app.update(Msg::RelatorioVba(Evento::AbrirModulo));
        assert!(!app.relatorio_vba.as_ref().unwrap().aberto);
        assert_eq!(app.macro_nome, modulo);
        let _ = app.update(Msg::RelatorioVba(Evento::Abrir));
        assert!(app.relatorio_vba.as_ref().unwrap().aberto, "o menu reabre o último relatório");
        let _ = app.update(Msg::RelatorioVba(Evento::Fechar));
        assert!(!app.relatorio_vba.as_ref().unwrap().aberto);

        // Abrir o .xlsm (sem pedir importação) guarda o relatório, mas não o joga na tela.
        app.instalar_vba("Vendas", &c, false);
        assert!(!app.relatorio_vba.as_ref().unwrap().aberto);
        for icone in ["file-code", "alert-triangle", "info-circle", "circle-check", "list-details"] {
            assert!(feather_ui::icones::existe(icone), "ícone {icone}");
        }
    }
}
