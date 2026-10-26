//! Menu backstage do FeatherSheet. Não altera a Ribbon da aba Arquivo.
//! Histórico usa feathersheet-io::recents, com persistência local por usuário.
use super::*;
use feathersheet_io::recents::{self, Recente};
use crate::compat::canvas; use pelicano::text_input; use pelicano::tooltip; use pelicano::Color; use pelicano::Point; use crate::compat::Rectangle; use pelicano::Size;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pagina { Inicio, Recentes, Exportar, Informacoes }

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Historico {
    pub(super) arquivos: Vec<Recente>,
    fixados: Vec<String>,
}

// Os testes nunca tocam o histórico real da máquina (ver `carregar`/`salvar`).
#[cfg_attr(test, allow(dead_code))]
fn arquivo_historico() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".config")))
        .or_else(|| {
            #[cfg(windows)]
            { std::env::var_os("APPDATA").map(PathBuf::from) }
            #[cfg(not(windows))]
            { None }
        })
        .map(|dir| dir.join("feathersheet/recents-iced.json"))
}
fn agora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_millis() as u64
}

impl Historico {
    #[cfg_attr(test, allow(dead_code))]
    fn ler(path: &Path) -> Self {
        let Ok(dados) = std::fs::read_to_string(path) else { return Self::default(); };
        let mut historico = serde_json::from_str::<Self>(&dados).unwrap_or_else(|_| Self {
            arquivos: recents::da_json(Some(&dados)), fixados: vec![],
        });
        historico.arquivos.truncate(recents::MAX_RECENTES);
        historico.fixados.retain(|p|
            historico.arquivos.iter().any(|a| &a.caminho == p));
        historico
    }
    pub(super) fn carregar() -> Self {
        #[cfg(test)]
        { Self::default() }
        #[cfg(not(test))]
        { arquivo_historico().map(|p| Self::ler(&p)).unwrap_or_default() }
    }
    fn salvar(&self) {
        #[cfg(not(test))]
        if let Some(path) = arquivo_historico() {
            if let Some(dir) = path.parent() {
                if std::fs::create_dir_all(dir).is_err() { return; }
            }
            if let Ok(bytes) = serde_json::to_vec_pretty(self) {
                let _ = feathersheet_io::arquivo::gravar_atomico(&path, &bytes);
            }
        }
    }
    pub(super) fn registrar(&mut self, path: &Path, abas: usize) {
        let nome = path.file_name().and_then(|v| v.to_str())
            .unwrap_or(tr!("Planilha")).to_owned();
        self.arquivos = recents::registrar(&self.arquivos, Recente {
            caminho: path.to_string_lossy().into_owned(),
            nome, aberto: agora(), abas: Some(abas),
        });
        self.fixados.retain(|p| self.arquivos.iter().any(|a| &a.caminho == p));
        self.salvar();
    }
    fn fixado(&self, p: &str) -> bool {
        self.fixados.iter().any(|nome| nome == p)
    }
    pub(super) fn alternar_fixado(&mut self, i: usize) {
        let Some(arquivo) = self.arquivos.get(i) else { return; };
        let path = arquivo.caminho.clone();
        if self.fixado(&path) { self.fixados.retain(|p| p != &path); }
        else { self.fixados.push(path); }
        self.salvar();
    }
    pub(super) fn esquecer(&mut self, i: usize) {
        if i >= self.arquivos.len() { return; }
        let path = self.arquivos.remove(i).caminho;
        self.fixados.retain(|p| p != &path);
        self.salvar();
    }
    fn ordem(&self) -> Vec<usize> {
        let mut indices: Vec<_> = (0..self.arquivos.len()).collect();
        indices.sort_by_key(|&i| !self.fixado(&self.arquivos[i].caminho));
        indices
    }
}

impl App {
    pub(super) fn registrar_arquivo_recente(&mut self, path: &Path) {
        self.recentes.registrar(path, self.workbook.len());
    }

    fn menu_item(&self, label: &'static str, icon: &'static str,
        evento: Option<Msg>, selecionado: bool,
    ) -> Element<'_, Msg> {
        let habilitado = evento.is_some();
        let linha = pelicano::row![
            feather_ui::icones::icone(icon, 18.0),
            text(label).size(13),
            Space::new(Length::FILL, 0.0),
        ].spacing(11).align(pelicano::Align::Center);
        button(linha).width(Length::FILL).padding([9, 13])
            .style(move |tema, estado| {
                let mut estilo = estilo_botao_secundario(tema, estado);
                estilo.border.radius = [9.0; 4];
                estilo.border.width = 0.0;
                estilo.background = if selecionado {
                    Some(pelicano::Background::Color(COR_AZUL_FUNDO()))
                } else { None };
                if selecionado { estilo.text_color = Some(COR_AZUL_ACENTO()); }
                else if !habilitado { estilo.text_color = Some(COR_TEXTO_MUTED()); }
                estilo
            }).on_press_maybe(evento).into()
    }
    fn menu_divisor(&self) -> Element<'_, Msg> {
        container(Space::new(0.0, Length::Fixed(1.0)))
            .width(Length::FILL).style(estilo_linha).into()
    }

    fn menu_lateral(&self) -> Element<'_, Msg> {
        let lista = pelicano::column![
            self.menu_item(tr!("Início"), "layout-sidebar",
                Some(Msg::MenuPagina(Pagina::Inicio)),
                self.menu_pagina == Pagina::Inicio),
            self.menu_item(tr!("Novo"), "file-plus", Some(Msg::Novo), false),
            self.menu_item(tr!("Abrir"), "folder-open", Some(Msg::Abrir), false),
            self.menu_item(tr!("Recentes"), "clock",
                Some(Msg::MenuPagina(Pagina::Recentes)),
                self.menu_pagina == Pagina::Recentes),
            self.menu_item(tr!("Recuperar temporário"), "clock",
                Some(Msg::RecuperarTemporario), false),
            self.menu_divisor(),
            self.menu_item(tr!("Salvar"), "device-floppy", Some(Msg::Salvar), false),
            self.menu_item(tr!("Salvar como..."), "device-floppy", Some(Msg::SalvarComo), false),
            self.menu_item(tr!("Fazer uma cópia"), "copy", Some(Msg::MenuSalvarCopia), false),
            self.menu_item(tr!("Renomear arquivo"), "edit", Some(Msg::RenomearArquivo), false),
            self.menu_divisor(),
            self.menu_item(tr!("Importar"), "folder-open", Some(Msg::Abrir), false),
            // Um item só: a página Exportar oferece Excel, CSV, TSV e JSON.
            self.menu_item(tr!("Exportar"), "file-export",
                Some(Msg::MenuPagina(Pagina::Exportar)),
                self.menu_pagina == Pagina::Exportar),
            self.menu_item(tr!("Imprimir"), "printer", Some(Msg::Impressao(EventoImpressao::Abrir)), false),
            self.menu_divisor(),
            self.menu_item(tr!("Compartilhar"), "list-details", None, false),
            self.menu_item(tr!("Informações"), "info-circle",
                Some(Msg::MenuPagina(Pagina::Informacoes)),
                self.menu_pagina == Pagina::Informacoes),
            self.menu_item(tr!("Opções"), "settings", Some(Msg::Configuracoes), false),
            self.menu_divisor(),
            self.menu_item(tr!("Fechar"), "x", Some(Msg::FecharAplicativo), false),
        ].spacing(3).padding(11);
        container(scrollable(lista).height(Length::FILL))
            .width(Length::Fixed(217.0)).height(Length::FILL)
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL_FAIXA())),
                ..Default::default()
            }).into()
    }

    fn menu_linha_recente(&self, i: usize) -> Element<'_, Msg> {
        let item = &self.recentes.arquivos[i];
        let existe = Path::new(&item.caminho).is_file();
        let nome = if item.nome.trim().is_empty() {
            Path::new(&item.caminho).file_name()
                .and_then(|v| v.to_str()).unwrap_or(tr!("Planilha")).to_owned()
        } else { item.nome.clone() };
        let detalhe = if existe { item.caminho.clone() }
            else { trf!("Arquivo não encontrado: {}", item.caminho) };
        let linha = button(pelicano::row![
            feather_ui::icones::icone("file-plus", 21.0),
            pelicano::column![
                text(nome).size(12),
                text(detalhe).size(10).color(COR_TEXTO_MUTED()),
            ].spacing(2).width(Length::FILL),
            text(recents::quando(item.aberto, agora())).size(10)
                .color(COR_TEXTO_MUTED()),
        ].spacing(10).align(pelicano::Align::Center))
            .width(Length::FILL).padding([7, 8])
            .style(estilo_botao_secundario)
            .on_press_maybe(existe.then_some(Msg::MenuAbrirRecente(i)));
        let marcado = self.recentes.fixado(&item.caminho);
        pelicano::row![
            linha,
            button(feather_ui::icones::icone(
                if marcado { "circle-check" } else { "plus" }, 17.0))
                .on_press(Msg::MenuFixarRecente(i))
                .style(estilo_botao_pequeno(marcado)),
            button(feather_ui::icones::icone("x", 15.0))
                .on_press(Msg::MenuEsquecerRecente(i))
                .style(estilo_botao_pequeno(false)),
        ].align(pelicano::Align::Center).spacing(3).into()
    }

    fn menu_lista_recentes(&self, limite: usize) -> Element<'_, Msg> {
        if self.recentes.arquivos.is_empty() {
            return container(pelicano::column![
                text(tr!("Ainda não há planilhas recentes.")).size(13),
                text(tr!("As planilhas abertas ou salvas aparecerão aqui."))
                    .size(11).color(COR_TEXTO_MUTED()),
                button(tr!("Abrir planilha")).on_press(Msg::Abrir)
                    .style(estilo_botao_primario),
            ].spacing(11)).padding(16).style(estilo_painel_faixa).into();
        }
        let mut lista = pelicano::column(Vec::<pelicano::Element<_>>::new()).spacing(6);
        for i in self.recentes.ordem().into_iter().take(limite) {
            lista = lista.push(self.menu_linha_recente(i));
        }
        lista.into()
    }

    fn menu_conteudo(&self) -> Element<'_, Msg> {
        match self.menu_pagina {
            Pagina::Inicio => self.inicio_conteudo(false),
            Pagina::Recentes => pelicano::column![
                text(tr!("Recentes")).size(24),
                text(tr!("Abra, fixe ou remova arquivos do histórico local."))
                    .size(12).color(COR_TEXTO_MUTED()),
                self.menu_lista_recentes(recents::MAX_RECENTES),
            ].spacing(12).into(),
            Pagina::Exportar => self.menu_exportar(),
            Pagina::Informacoes => pelicano::column![
                text(tr!("Informações")).size(24),
                text(self.nome_documento()).size(17),
                text(self.caminho.as_ref().map(|p| p.display().to_string())
                    .unwrap_or_else(|| tr!("Documento não salvo em .fshz").into()))
                    .size(12).color(COR_TEXTO_MUTED()),
                self.menu_divisor(),
                text(trf!("Abas: {}", self.workbook.len())).size(13),
                text(if self.alterado { tr!("Há alterações não salvas") }
                    else { tr!("Sem alterações pendentes") }).size(13),
                pelicano::row![
                    button(tr!("Salvar")).on_press(Msg::Salvar).style(estilo_botao_primario),
                    button(tr!("Salvar como...")).on_press(Msg::SalvarComo)
                        .style(estilo_botao_secundario),
                ].spacing(8),
            ].spacing(13).into(),
        }
    }

    /// Página Exportar: um cartão por formato; o cartão abre o diálogo de salvar
    /// já com a extensão certa (o filtro do diálogo não troca a extensão do nome).
    fn menu_exportar(&self) -> Element<'_, Msg> {
        let negrito = pelicano::Font { weight: 600, ..pelicano::Font::DEFAULT };
        let cartao = |icone: &'static str, titulo: &'static str, extensao: &'static str,
            descricao: &'static str, evento: Msg| -> Element<'static, Msg> {
            let simbolo = container(feather_ui::icones::icone(icone, 22.0))
                .padding(11).style(|_| pelicano::Style {
                    background: Some(pelicano::Background::Color(COR_AZUL_FUNDO())),
                    border: pelicano::Border { radius: [10.0; 4], ..Default::default() },
                    ..Default::default()
                });
            button(pelicano::row![
                simbolo,
                pelicano::column![
                    pelicano::row![
                        text(titulo).size(13.5).font(negrito),
                        text(extensao).size(12).color(COR_TEXTO_MUTED()),
                    ].spacing(6).align(pelicano::Align::Center),
                    text(descricao).size(11.5).color(COR_TEXTO_MUTED()),
                ].spacing(3).width(Length::FILL),
                feather_ui::icones::icone("chevron-right", 16.0),
            ].spacing(12).align(pelicano::Align::Center))
                .width(Length::FILL).padding([11, 13])
                .style(|_tema, estado| {
                    let sobre = matches!(estado, pelicano::Status::Hovered);
                    pelicano::Style {
                        background: Some(pelicano::Background::Color(if sobre { COR_AZUL_FUNDO() } else { COR_PAINEL() })),
                        text_color: Some(COR_TEXTO()),
                        border: pelicano::Border {
                            color: if sobre { COR_AZUL_ACENTO() } else { COR_BORDA() },
                            width: 1.0, radius: [11.0; 4],
                        },
                        ..Default::default()
                    }
                })
                .on_press(evento).into()
        };
        pelicano::column![
            text(tr!("Exportar")).size(24).font(negrito),
            text(tr!("Salva uma cópia em outro formato. O arquivo .fshz em que você trabalha não muda."))
                .size(12).color(COR_TEXTO_MUTED()),
            cartao("file-export", tr!("Excel"), ".xlsx",
                tr!("Todas as abas, para abrir no Excel ou em outro programa de planilhas."), Msg::ExportarXlsx),
            cartao("file-export", "CSV", ".csv",
                tr!("Só a aba atual, valores separados por vírgula."), Msg::ExportarDelimitado(false)),
            cartao("file-code", "TSV", ".tsv",
                tr!("Só a aba atual, valores separados por tabulação."), Msg::ExportarDelimitado(true)),
            cartao("file-code", "JSON", ".json",
                tr!("Todas as abas, para integrar com outros programas."), Msg::ExportarJson),
        ].spacing(12).into()
    }

    /// Abre em cima da Ribbon e da grade, abaixo das abas; nada encolhe.
    pub(super) fn sobrepor_menu_principal<'a>(
        &'a self, base: Element<'a, Msg>
    ) -> Element<'a, Msg> {
        if !self.menu_aberto { return base; }
        let largura = (self.tamanho_principal.width - 20.0).min(885.0).max(310.0);
        let y = if self.config.barra_propria { 95.0 } else { 52.0 };
        let altura = (self.tamanho_principal.height - y - 12.0).max(175.0);
        let painel = container(pelicano::row![
            self.menu_lateral(),
            container(scrollable(pelicano::column![
                self.menu_conteudo()
            ].padding([23, 23])).height(Length::FILL))
                .width(Length::FILL).height(Length::FILL),
        ].height(Length::FILL))
            .width(Length::Fixed(largura)).height(Length::Fixed(altura))
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL())),
                border: pelicano::Border {
                    color: COR_BORDA(), width: 1.0, radius: [13.0; 4],
                },
                ..Default::default()
            });
        let fora = pelicano::mouse_area(
            Space::new(Length::FILL, Length::FILL)
        ).on_press(Msg::FecharMenu);
        stack![
            base,
            fora,
            pelicano::column![
                Space::new(0.0, Length::Fixed(y)),
                pelicano::row![
                    Space::new(Length::Fixed(12.0), 0.0),
                    painel,
                ],
            ],
        ].height(Length::FILL).into()
    }
}

/// Motor de modelos: `feathersheet_core::modelos` (`lista`, `criar`, `ModeloInfo`).
pub(super) use feathersheet_core::modelos;

/// Largura e altura fixas dos cartões de modelo: todos iguais, nome numa linha.
const CARTAO_LARGURA: f32 = 196.0;
const CARTAO_ALTURA: f32 = 224.0;
const CARTAO_ESPACO: f32 = 14.0;
const MINIATURA_ALTURA: f32 = 124.0;
/// Setas do carrossel: botões redondos pequenos.
const SETA: f32 = 32.0;
/// Largura máxima do conteúdo em telas largas (o resto fica de margem).
const CONTEUDO_MAX: f32 = 1180.0;
/// Colunas fixas da lista de recentes.
const COL_TIPO: f32 = 46.0;
const COL_QUANDO: f32 = 104.0;
const COL_ACOES: f32 = 62.0;
/// Miniatura da pasta em branco: grade vazia com um "+" no meio. Os outros
/// modelos usam a planilha de verdade (`miniatura_modelo`).
#[derive(Clone, Copy)]
struct MiniaturaPlanilha;

impl<M> canvas::Program<M> for MiniaturaPlanilha {
    type State = ();
    fn draw(
        &self, _state: &(), renderer: &mut pelicano::Renderer, bounds: Rectangle,
        _theme: &pelicano::Theme, _cursor: Option<Point>,
    ) {
        let mut f = canvas::Frame::new(renderer, bounds);
        let (w, h) = (bounds.width, bounds.height);
        f.fill_rectangle(Point::ORIGIN, bounds.size(), COR_PAINEL());
        let traco = |cor| canvas::Stroke::default().with_color(cor).with_width(1.0);
        let (colunas, linhas) = (4, 6);
        for c in 1..colunas {
            let x = (w * c as f32 / colunas as f32).round() + 0.5;
            f.stroke(&canvas::Path::line(Point::new(x, 0.0), Point::new(x, h)), traco(COR_BORDA_SUAVE()));
        }
        for r in 1..linhas {
            let y = (h * r as f32 / linhas as f32).round() + 0.5;
            f.stroke(&canvas::Path::line(Point::new(0.0, y), Point::new(w, y)), traco(COR_BORDA_SUAVE()));
        }
        let (cx, cy) = (w / 2.0, h / 2.0);
        f.fill(&canvas::Path::circle(Point::new(cx, cy), 17.0), COR_AZUL_ACENTO());
        let mais = canvas::Stroke::default().with_color(Color::WHITE).with_width(2.5);
        f.stroke(&canvas::Path::line(Point::new(cx - 8.0, cy), Point::new(cx + 8.0, cy)), mais.clone());
        f.stroke(&canvas::Path::line(Point::new(cx, cy - 8.0), Point::new(cx, cy + 8.0)), mais);
        f.stroke(&canvas::Path::rectangle(Point::new(0.5, 0.5), Size::new(w - 1.0, h - 1.0)),
            traco(COR_BORDA()));
        f.fim();
    }
}

/// Corta `s` em `max` caracteres com reticências no fim.
fn cortar_fim(s: &str, max: usize) -> String {
    if s.chars().count() <= max.max(2) { return s.to_owned(); }
    let mut curto: String = s.chars().take(max.max(2) - 1).collect();
    curto.push('…');
    curto
}

/// Corta `s` em `max` caracteres mantendo o fim (o fim de uma pasta diz mais).
fn cortar_inicio(s: &str, max: usize) -> String {
    let total = s.chars().count();
    if total <= max.max(2) { return s.to_owned(); }
    let resto: String = s.chars().skip(total - (max.max(2) - 1)).collect();
    format!("…{resto}")
}

/// Pasta do arquivo, com a pasta pessoal abreviada para `~`.
fn pasta_legivel(caminho: &str) -> String {
    let pasta = Path::new(caminho).parent()
        .map(|p| p.display().to_string()).unwrap_or_default();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && pasta.starts_with(&home) => format!("~{}", &pasta[home.len()..]),
        _ => pasta,
    }
}

/// Selo do tipo de arquivo: rótulo e cor (verde Excel, laranja texto, cor da suíte).
fn selo_tipo(caminho: &str) -> (String, Color) {
    let ext = Path::new(caminho).extension().and_then(|e| e.to_str())
        .unwrap_or("").to_ascii_uppercase();
    let cor = match ext.as_str() {
        "XLSX" | "XLSM" | "XLS" => Color::rgb8(0x1D, 0x6F, 0x42),
        "CSV" | "TSV" => Color::rgb8(0xC2, 0x6A, 0x00),
        "FSHZ" => COR_AZUL_ACENTO(),
        _ => COR_TEXTO_MUTED(),
    };
    // O selo tem largura de quatro letras: "PARQUET" quebrava em duas linhas.
    let rotulo = if ext.is_empty() { "—".into() } else { ext.chars().take(4).collect() };
    (rotulo, cor)
}

fn negrito() -> pelicano::Font {
    pelicano::Font { weight: 600, ..pelicano::Font::DEFAULT }
}

impl App {
    /// Largura útil da área de conteúdo: a janela inteira (tela inicial) ou o painel do backstage.
    fn largura_util_inicio(&self, tela_cheia: bool) -> f32 {
        if tela_cheia {
            return (self.tamanho_principal.width - 88.0).clamp(CARTAO_LARGURA * 2.0, CONTEUDO_MAX);
        }
        let largura = (self.tamanho_principal.width - 20.0).min(885.0).max(310.0);
        (largura - 217.0 - 46.0).max(CARTAO_LARGURA)
    }

    /// Cartão de modelo: miniatura, nome numa linha e descrição em até duas.
    fn inicio_cartao(&self, id: &str, nome: &str, detalhe: &str,
        evento: Msg,
    ) -> Element<'_, Msg> {
        let interno = CARTAO_LARGURA - 16.0;
        // A miniatura é a planilha de verdade que o modelo cria; o em branco ganha o "+".
        let desenho: Element<'_, Msg> = match super::miniatura_modelo::quadro(id, interno / MINIATURA_ALTURA) {
            Some(q) if id != "em-branco" => canvas(super::miniatura_modelo::Miniatura(q))
                .width(Length::FILL).height(Length::FILL).into(),
            _ => canvas(MiniaturaPlanilha).width(Length::FILL).height(Length::FILL).into(),
        };
        button(pelicano::column![
            container(desenho).width(Length::Fixed(interno)).height(Length::Fixed(MINIATURA_ALTURA)),
            text(cortar_fim(nome, 44)).size(13).font(negrito()).width(Length::Fixed(interno))
                .height(Length::Fixed(34.0)),
            text(cortar_fim(detalhe, 68)).size(11).color(COR_TEXTO_MUTED())
                .width(Length::Fixed(interno)),
        ].spacing(5))
            .padding(8)
            .width(Length::Fixed(CARTAO_LARGURA))
            .height(Length::Fixed(CARTAO_ALTURA))
            .style(|_tema, estado| {
                let sobre = matches!(estado, pelicano::Status::Hovered);
                pelicano::Style {
                    background: Some(pelicano::Background::Color(if sobre { COR_HOVER() } else { COR_PAINEL() })),
                    text_color: Some(COR_TEXTO()),
                    border: pelicano::Border {
                        color: if sobre { COR_AZUL_ACENTO() } else { COR_BORDA_SUAVE() },
                        width: 1.0,
                        radius: [8.0; 4],
                    },
                    shadow: Some(pelicano::Shadow {
                        color: Color::rgb8(0, 0, 0).alpha(if sobre { 0.14 } else { 0.06 }),
                        offset: Point::new(0.0, if sobre { 2.0 } else { 1.0 }),
                        blur: if sobre { 8.0 } else { 3.0 },
                    }),
                    ..Default::default()
                }
            })
            .on_press(evento)
            .into()
    }

    /// Seta redonda do carrossel; `ativa` desliga a seta nas pontas.
    fn seta_carrossel(nome: &'static str, d: i32, ativa: bool) -> Element<'static, Msg> {
        button(container(feather_ui::icones::icone(nome, 16.0))
            .center())
            .width(Length::Fixed(SETA)).height(Length::Fixed(SETA))
            .padding(0)
            .style(move |tema, estado| {
                let mut estilo = estilo_botao_secundario(tema, estado);
                estilo.border.radius = [SETA / 2.0; 4];
                if !ativa { estilo.background = None; estilo.border.width = 0.0; }
                estilo
            })
            .on_press_maybe(ativa.then_some(Msg::CarrosselModelos(d)))
            .into()
    }

    /// Botão com cara de link, para "Ver todos ›" e afins.
    fn botao_link(rotulo: String, icone: &'static str, evento: Msg) -> Element<'static, Msg> {
        button(row![
            text(rotulo).size(12.5).color(COR_AZUL_ACENTO()),
            feather_ui::icones::icone(icone, 14.0)
                .color(COR_AZUL_ACENTO()),
        ].spacing(3).align(pelicano::Align::Center))
            .padding([3, 6])
            .style(|tema, estado| {
                let mut estilo = estilo_botao_secundario(tema, estado);
                estilo.border.width = 0.0;
                if !matches!(estado, pelicano::Status::Hovered) { estilo.background = None; }
                estilo
            })
            .on_press(evento)
            .into()
    }

    /// Faixa "Novo": modelos num carrossel de cartões iguais, ou todos em grade.
    fn inicio_galeria(&self, largura: f32) -> Element<'_, Msg> {
        let modelos_lista = modelos::lista();
        let total = modelos_lista.len();
        if total == 0 {
            return text(tr!("Nenhum modelo disponível neste build.")).size(12).color(COR_TEXTO_MUTED()).into();
        }
        let cartoes: Vec<Element<'_, Msg>> = modelos_lista.iter().map(|m|
            self.inicio_cartao(m.id, m.nome, m.descricao, Msg::NovoDeModelo(m.id))
        ).collect();
        let passo = CARTAO_LARGURA + CARTAO_ESPACO;
        let cabem = ((largura + CARTAO_ESPACO) / passo).floor().max(1.0) as usize;
        // Com setas, cada uma ocupa SETA + espaço dos dois lados.
        let visiveis = if cabem >= total { total } else {
            (((largura - 2.0 * (SETA + CARTAO_ESPACO) + CARTAO_ESPACO) / passo).floor().max(1.0) as usize).min(total)
        };
        let inicio = self.carrossel_modelos.min(total - visiveis);
        let rodando = !self.ver_todos_modelos && visiveis < total;
        let mut cabecalho = pelicano::row![
            text(tr!("Novo")).size(16).font(negrito()),
            Space::new(Length::FILL, 0.0),
        ].spacing(10).align(pelicano::Align::Center);
        if rodando {
            cabecalho = cabecalho.push(text(trf!("{}–{} de {}", inicio + 1, inicio + visiveis, total))
                .size(11.5).color(COR_TEXTO_MUTED()));
        }
        if visiveis < total || self.ver_todos_modelos {
            cabecalho = cabecalho.push(Self::botao_link(
                if self.ver_todos_modelos { tr!("Mostrar menos").into() } else { tr!("Ver todos os modelos").into() },
                if self.ver_todos_modelos { "chevron-up" } else { "chevron-right" },
                Msg::VerTodosModelos));
        }
        let corpo: Element<'_, Msg> = if !rodando {
            let por_linha = cabem.max(1);
            let mut grade = pelicano::column(Vec::<pelicano::Element<_>>::new()).spacing(CARTAO_ESPACO);
            let mut restantes = cartoes.into_iter().peekable();
            while restantes.peek().is_some() {
                let mut linha = pelicano::row(Vec::<pelicano::Element<_>>::new()).spacing(CARTAO_ESPACO);
                for cartao in restantes.by_ref().take(por_linha) {
                    linha = linha.push(cartao);
                }
                grade = grade.push(linha);
            }
            grade.into()
        } else {
            let mut faixa = pelicano::row![
                Self::seta_carrossel("chevron-left", -1, inicio > 0),
            ].spacing(CARTAO_ESPACO).align(pelicano::Align::Center);
            for cartao in cartoes.into_iter().skip(inicio).take(visiveis) {
                faixa = faixa.push(cartao);
            }
            faixa.push(Self::seta_carrossel("chevron-right", 1, inicio + visiveis < total)).into()
        };
        pelicano::column![cabecalho, corpo].spacing(12).into()
    }

    /// Uma linha da lista de recentes: selo do tipo, nome, pasta, quando, fixar e remover.
    fn inicio_linha_recente(&self, i: usize, largura: f32) -> Element<'_, Msg> {
        let item = &self.recentes.arquivos[i];
        let existe = Path::new(&item.caminho).is_file();
        let nome = if item.nome.trim().is_empty() {
            Path::new(&item.caminho).file_name()
                .and_then(|v| v.to_str()).unwrap_or(tr!("Planilha")).to_owned()
        } else { item.nome.clone() };
        // As colunas Nome e Pasta dividem o que sobra em 5:4; ~7 px por caractere.
        let livre = (largura - COL_TIPO - COL_QUANDO - COL_ACOES - 40.0).max(120.0);
        let max_nome = (livre * 5.0 / 9.0 / 7.2) as usize;
        let max_pasta = (livre * 4.0 / 9.0 / 6.6) as usize;
        let (rotulo, cor) = selo_tipo(&item.caminho);
        let selo = container(text(rotulo).size(9.5).font(negrito()).color(Color::WHITE))
            .padding([2, 5])
            .style(move |_| pelicano::Style {
                background: Some(pelicano::Background::Color(if existe { cor } else { COR_TEXTO_MUTED() })),
                border: pelicano::Border { radius: [4.0; 4], ..Default::default() },
                ..Default::default()
            });
        let fixado = self.recentes.fixado(&item.caminho);
        let mut nome_txt = text(cortar_fim(&nome, max_nome)).size(13);
        if !existe { nome_txt = nome_txt.color(COR_TEXTO_MUTED()); }
        let pasta = if existe { cortar_inicio(&pasta_legivel(&item.caminho), max_pasta) }
            else { tr!("Arquivo não encontrado").to_owned() };
        let linha = button(pelicano::row![
            container(selo).width(Length::Fixed(COL_TIPO)),
            container(nome_txt).width(Length::Fill(5)),
            container(text(pasta).size(12).color(COR_TEXTO_MUTED())).width(Length::Fill(4)),
            container(text(recents::quando(item.aberto, agora())).size(12).color(COR_TEXTO_MUTED()))
                .width(Length::Fixed(COL_QUANDO)),
        ].spacing(10).align(pelicano::Align::Center))
            .width(Length::FILL)
            .padding([8, 10])
            .style(|_tema, estado| pelicano::Style {
                background: matches!(estado, pelicano::Status::Hovered)
                    .then(|| pelicano::Background::Color(COR_HOVER())),
                text_color: Some(COR_TEXTO()),
                border: pelicano::Border { radius: [6.0; 4], ..Default::default() },
                ..Default::default()
            })
            .on_press_maybe(existe.then_some(Msg::MenuAbrirRecente(i)));
        let pequeno = |icone: &'static str, ativo: bool, evento: Msg| {
            button(container(feather_ui::icones::icone(icone, 15.0)).center())
                .width(Length::Fixed(26.0)).height(Length::Fixed(26.0)).padding(0)
                .style(move |tema, estado| {
                    let mut estilo = estilo_botao_pequeno(ativo)(tema, estado);
                    estilo.border.radius = [13.0; 4];
                    if !ativo && !matches!(estado, pelicano::Status::Hovered) {
                        estilo.background = None;
                        estilo.border.width = 0.0;
                    }
                    estilo
                })
                .on_press(evento)
        };
        pelicano::row![
            linha,
            pelicano::row![
                tooltip(pequeno(if fixado { "circle-check" } else { "plus" }, fixado, Msg::MenuFixarRecente(i)),
                    if fixado { tr!("Desafixar") } else { tr!("Fixar no topo") },
                    pelicano::TooltipPosition::Top).style(estilo_dica),
                tooltip(pequeno("x", false, Msg::MenuEsquecerRecente(i)),
                    tr!("Tirar da lista"), pelicano::TooltipPosition::Top).style(estilo_dica),
            ].spacing(4).width(Length::Fixed(COL_ACOES)),
        ].spacing(4).align(pelicano::Align::Center).into()
    }

    /// Recentes em lista com busca. Na tela inicial cheia mostra todos; no backstage, seis e "Ver todos".
    fn inicio_recentes(&self, largura: f32, tela_cheia: bool) -> Element<'_, Msg> {
        let busca = self.busca_recentes.trim().to_lowercase();
        let mut titulo = pelicano::row![
            text(tr!("Recentes")).size(16).font(negrito()),
            Space::new(Length::FILL, 0.0),
        ].spacing(10).align(pelicano::Align::Center);
        if !self.recentes.arquivos.is_empty() {
            // Busca no padrão da Ajuda (F1): ícone `search` do catálogo + campo sem borda própria.
            titulo = titulo.push(container(row![
                feather_ui::icones::icone("search", 15.0),
                text_input(tr!("Buscar nos recentes"), &self.busca_recentes)
                    .on_input(Msg::BuscarRecentes)
                    .size(13).padding([5, 4])
                    .style(|tema, estado| {
                        let mut e = pelicano::input_default(tema, estado);
                        e.background = Some(pelicano::Background::Color(COR_PAINEL()));
                        e.border = pelicano::Border::default();
                        e
                    }),
            ].spacing(6).align(pelicano::Align::Center))
                .padding([1, 10])
                .width(Length::Fixed((largura * 0.35).clamp(170.0, 280.0)))
                .style(|_| pelicano::Style {
                    background: Some(pelicano::Background::Color(COR_PAINEL())),
                    border: pelicano::Border { color: COR_BORDA(), width: 1.0, radius: [8.0; 4] },
                    ..Default::default()
                }));
        }
        if !tela_cheia && self.recentes.arquivos.len() > 6 {
            titulo = titulo.push(Self::botao_link(tr!("Ver todos").into(), "chevron-right", Msg::MenuPagina(Pagina::Recentes)));
        }
        if self.recentes.arquivos.is_empty() {
            return pelicano::column![
                titulo,
                container(pelicano::column![
                    feather_ui::icones::icone("folder-open", 30.0),
                    text(tr!("Nenhuma planilha aberta ainda")).size(13),
                    text(tr!("As planilhas que você abrir ou salvar aparecem aqui."))
                        .size(11.5).color(COR_TEXTO_MUTED()),
                ].spacing(6).align(pelicano::Align::Center))
                    .width(Length::FILL).align_x(pelicano::Align::Center).padding(28),
            ].spacing(10).into();
        }
        let limite = if tela_cheia { recents::MAX_RECENTES } else { 6 };
        let indices: Vec<usize> = self.recentes.ordem().into_iter()
            .filter(|&i| busca.is_empty() || {
                let a = &self.recentes.arquivos[i];
                a.nome.to_lowercase().contains(&busca) || a.caminho.to_lowercase().contains(&busca)
            })
            .take(limite).collect();
        let rotulo_coluna = |s: &'static str| text(s).size(11).color(COR_TEXTO_MUTED());
        let colunas = container(pelicano::row![
            Space::new(Length::Fixed(COL_TIPO), 0.0),
            container(rotulo_coluna(tr!("Nome"))).width(Length::Fill(5)),
            container(rotulo_coluna(tr!("Pasta"))).width(Length::Fill(4)),
            container(rotulo_coluna(tr!("Aberto"))).width(Length::Fixed(COL_QUANDO)),
            Space::new(Length::Fixed(COL_ACOES + 4.0), 0.0),
        ].spacing(10)).padding([0, 10]);
        let mut lista = pelicano::column![colunas, self.menu_divisor()].spacing(4);
        if indices.is_empty() {
            lista = lista.push(container(text(trf!("Nenhum recente com “{}”.", self.busca_recentes.trim()))
                .size(12).color(COR_TEXTO_MUTED())).padding([14, 10]));
        }
        for i in indices {
            lista = lista.push(self.inicio_linha_recente(i, largura));
        }
        pelicano::column![titulo, lista].spacing(10).into()
    }

    /// Tela inicial: marca e Abrir, faixa "Novo" com os modelos e a lista de recentes.
    /// Serve ao backstage (`tela_cheia = false`, dentro de uma rolagem) e à janela
    /// inteira na abertura sem arquivo.
    fn inicio_conteudo(&self, tela_cheia: bool) -> Element<'_, Msg> {
        let largura = self.largura_util_inicio(tela_cheia);
        let cabecalho = pelicano::row![
            feather_ui::marca::icone_atual(if tela_cheia { 44.0 } else { 36.0 }),
            pelicano::column![
                text("FeatherSheet").size(if tela_cheia { 24 } else { 20 }).font(negrito()),
                text(tr!("Planilhas que ficam na sua máquina.")).size(12.5).color(COR_TEXTO_MUTED()),
            ].spacing(1),
            Space::new(Length::FILL, 0.0),
            button(pelicano::row![
                feather_ui::icones::icone("folder-open", 16.0),
                text(tr!("Abrir…")).size(13),
            ].spacing(7).align(pelicano::Align::Center))
                .padding([8, 16]).style(estilo_botao_secundario).on_press(Msg::Abrir),
        ].spacing(12).align(pelicano::Align::Center);

        // Os modelos ficam numa faixa com fundo de painel, como a do Excel/Sheets.
        let faixa = container(self.inicio_galeria(largura - 36.0))
            .padding([14, 18])
            .width(Length::FILL)
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL_FAIXA())),
                border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [10.0; 4] },
                ..Default::default()
            });
        let recentes = self.inicio_recentes(largura, tela_cheia);
        let recentes: Element<'_, Msg> = match self.cartao_recuperacao() {
            Some(cartao) => pelicano::column![cartao, recentes].spacing(16).into(),
            None => recentes,
        };

        if tela_cheia {
            // Só os recentes rolam; cabeçalho e modelos ficam fixos na janela.
            // O Iced 0.13 entra em panic se o conteúdo de uma rolagem vertical
            // anunciar altura Fill: o conteúdo mede a própria altura.
            return pelicano::column![
                cabecalho,
                faixa,
                feather_ui::tema::rolagem(container(recentes).height(Length::Shrink).padding([0, 2]))
                    .height(Length::FILL),
            ].spacing(18).width(Length::Fixed(largura)).height(Length::FILL).into();
        }
        pelicano::column![cabecalho, faixa, recentes].spacing(16).into()
    }

    /// Tela inicial em janela inteira (sem faixa nem grade), enquanto nenhuma planilha foi escolhida.
    pub(super) fn view_tela_inicio(&self, id: window::Id) -> Element<'_, Msg> {
        let corpo: Element<'_, Msg> = container(self.inicio_conteudo(true))
            .padding([24, 44])
            .width(Length::FILL).align_x(pelicano::Align::Center)
            .height(Length::FILL)
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_FUNDO_JANELA())),
                ..Default::default()
            })
            .into();
        if self.config.barra_propria {
            cromo::janela_com_barra(&self.titulo(id), corpo, Self::acoes(id))
        } else {
            corpo
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn menu_usa_inicio_mas_preserva_catalogo() {
        let mut app = App::default();
        let antes = app.faixa_ativa;
        app.update(Msg::Menu);
        assert!(app.menu_aberto);
        assert_eq!(app.menu_pagina, Pagina::Inicio);
        let _: Element<'_, Msg> = app.menu_conteudo();
        assert_eq!(app.faixa_ativa, antes);
        app.update(Msg::FecharMenu);
        // Faixa recolhida: o menu clássico abre a lista sem abrir o menu principal.
        app.update(Msg::MenuGrupo(1));
        assert!(app.barra_menu_aberta && !app.menu_aberto);
        let _: Element<'_, Msg> = app.painel_menu_classico();
        app.update(Msg::MenuVoltar);
        assert!(!app.barra_menu_aberta);
    }

    #[test]
    fn recentes_sao_reais_fixaveis_e_removiveis() {
        let mut h = Historico::default();
        h.registrar(Path::new("/tmp/teste-a.fshz"), 2);
        h.registrar(Path::new("/tmp/teste-b.xlsx"), 1);
        assert_eq!(h.ordem(), vec![0,1]);
        h.alternar_fixado(1);
        assert_eq!(h.ordem(), vec![1,0]);
        let json = serde_json::to_string(&h).unwrap();
        let outra: Historico = serde_json::from_str(&json).unwrap();
        assert!(outra.fixado("/tmp/teste-a.fshz"));
        h.esquecer(1);
        assert_eq!(h.arquivos.len(), 1);
        assert!(h.fixados.is_empty());
    }


    #[test]
    fn descartar_para_abrir_nao_apaga_flag_antes_de_substituir_documento() {
        let mut app = App::default();
        app.alterado = true;
        app.pendente = Some(Pendente::Abrir);
        let _ = app.update(Msg::Descartar);
        assert!(app.alterado, "cancelar a abertura não pode apagar o aviso de alterações");
        assert!(app.pendente.is_none());
    }

    #[test]
    fn os_icones_da_confirmacao_estao_no_catalogo_svg() {
        for nome in ["info-circle", "file-plus", "device-floppy", "clock"] {
            assert!(feather_ui::icones::existe(nome), "ícone ausente: {nome}");
        }
    }
    #[test]
    fn arquivo_recente_escolhido_nao_muda_apos_salvar_atual() {
        let mut app = App::default();
        app.recentes.registrar(Path::new("/tmp/a.fshz"), 1);
        app.recentes.registrar(Path::new("/tmp/b.fshz"), 1);
        app.alterado = true;
        app.update(Msg::MenuAbrirRecente(1));
        assert!(matches!(app.pendente, Some(Pendente::Recente)));
        assert_eq!(app.recente_destino.as_deref(), Some(Path::new("/tmp/a.fshz")));
        app.recentes.registrar(Path::new("/tmp/c.fshz"), 1);
        assert_eq!(app.recente_destino.as_deref(), Some(Path::new("/tmp/a.fshz")));
    }

    #[test]
    fn salvar_copia_nao_muda_caminho_ativo_nem_flag_de_alteracoes() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "42".into());
        app.alterado = true;
        let original = PathBuf::from("/tmp/original.fshz");
        app.caminho = Some(original.clone());
        let destino = std::env::temp_dir().join(format!(
            "feathersheet-copia-{}-{}.fshz", std::process::id(), agora()
        ));
        let _ = app.salvar_copia(destino.clone());
        app.concluir_gravacao_teste(destino.clone(), true);
        assert_eq!(app.caminho, Some(original));
        assert!(app.alterado);
        assert!(destino.is_file());
        assert!(app.recentes.arquivos.iter()
            .any(|r| Path::new(&r.caminho) == destino.as_path()));
        std::fs::remove_file(destino).unwrap();
    }

    #[test]
    fn atalhos_e_visao_inicial_nao_alteram_documento() {
        let mut app = App::default();
        assert!(app.recentes.arquivos.is_empty());
        let nomes = app.workbook.nomes();
        app.update(Msg::Menu);
        let _: Element<'_, Msg> = app.menu_conteudo();
        app.update(Msg::FecharMenu);
        assert_eq!(app.workbook.nomes(), nomes);
        assert!(!app.alterado);
    }

    #[test]
    fn app_sem_arquivo_comeca_na_tela_inicial_e_novo_sai_dela() {
        let (mut app, _) = App::novo_com(None);
        assert!(app.em_tela_inicio);
        let _: Element<'_, Msg> = app.view_tela_inicio(window::novo_id());
        // Regressão: com arquivos recentes há uma rolagem vertical na tela inicial.
        // Seu conteúdo não pode ter altura Fill (panic em iced_widget 0.13).
        app.recentes.registrar(Path::new("/tmp/feathersheet-teste-recente.fshz"), 1);
        let _: Element<'_, Msg> = app.view_tela_inicio(window::novo_id());
        app.ver_todos_modelos = true;
        let _: Element<'_, Msg> = app.view_tela_inicio(window::novo_id());
        let _ = app.update(Msg::Novo);
        assert!(!app.em_tela_inicio);
    }

    /// Pedido de 16/10: "Abrir…" um .csv na tela inicial ficava sem resposta.
    /// Escolhido o arquivo, a tela inicial sai na hora e o rodapé mostra a leitura.
    #[test]
    fn abrir_na_tela_inicial_sai_dela_e_mostra_o_carregamento() {
        let (mut app, _) = App::novo_com(None);
        assert!(app.em_tela_inicio);
        let _ = app.update(Msg::ArquivoEscolhido(Some(PathBuf::from("/tmp/feathersheet-teste.csv"))));
        assert!(!app.em_tela_inicio);
        assert!(app.carregando);
        let _: Element<'_, Msg> = app.view_progresso_abertura();
    }

    #[test]
    fn app_com_arquivo_na_linha_de_comando_vai_direto_a_planilha() {
        let (app, _) = App::novo_com(Some(PathBuf::from("/tmp/inexistente.fshz")));
        assert!(!app.em_tela_inicio);
    }

    #[test]
    fn carrossel_de_modelos_respeita_as_pontas_e_alterna_grade() {
        let mut app = App::default();
        let total = modelos::lista().len();
        let _ = app.update(Msg::CarrosselModelos(-1));
        assert_eq!(app.carrossel_modelos, 0);
        for _ in 0..total + 2 {
            let _ = app.update(Msg::CarrosselModelos(1));
        }
        assert_eq!(app.carrossel_modelos, total.saturating_sub(1));
        let _ = app.update(Msg::VerTodosModelos);
        assert!(app.ver_todos_modelos);
        let _: Element<'_, Msg> = app.inicio_conteudo(true);
        let _: Element<'_, Msg> = app.inicio_conteudo(false);
    }
}
