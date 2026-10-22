//! Imagem dentro da célula × imagem solta (17/10/2026), como o "Colocar na
//! célula" e o "Colocar sobre as células" do Excel. Dentro da célula, a
//! imagem é o texto `data:image/…;base64,…` (o que o leitor de `.xlsx` e a
//! fórmula `IMAGEM` produzem, e que a grade desenha); solta, é um item do
//! bloco `im` da aba ([`imagens_importadas`]). Cada troca é um só Ctrl+Z.
use super::{dialogos, imagens_importadas, App, Msg};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use feathersheet_core::celulas::refs::cell_name;
use feathersheet_core::funcoes::imagem_base64::eh_imagem_na_celula;
use pelicano::Task;

#[derive(Debug, Clone)]
pub enum Evento {
    /// Escolher um arquivo e pôr a figura na célula ativa.
    Inserir,
    Carregada(Result<Option<(Vec<u8>, String)>, String>),
    /// A imagem solta selecionada entra na célula debaixo do canto dela.
    ColocarNaCelula,
    /// A imagem da célula ativa vira imagem solta, e a célula fica vazia.
    ColocarSobre,
    /// Botão direito na imagem solta: seleciona e abre o menu dela.
    Contexto(String),
    FecharContexto,
    /// O mouse entrou num item do menu (o menu comum avisa para abrir
    /// submenu; este não tem): não faz nada.
    PassouNoMenu,
    /// Exclui a imagem solta selecionada.
    Excluir,
}

/// O tipo da figura para o data URL; `None` quando não é imagem que a grade
/// desenha.
fn tipo(bytes: &[u8]) -> Option<&'static str> {
    use image::ImageFormat as F;
    match image::guess_format(bytes).ok()? {
        F::Png => Some("png"),
        F::Jpeg => Some("jpeg"),
        F::Gif => Some("gif"),
        F::Bmp => Some("bmp"),
        F::WebP => Some("webp"),
        _ => None,
    }
}

/// Os bytes de um data URL de imagem.
fn bytes_do_texto(texto: &str) -> Option<Vec<u8>> {
    let (cabeca, corpo) = texto.split_once(',')?;
    if !cabeca.starts_with("data:image/") || !cabeca.ends_with(";base64") {
        return None;
    }
    let limpo: String = corpo.chars().filter(|c| !c.is_whitespace()).collect();
    STANDARD.decode(limpo).ok()
}

/// A imagem solta só aceita PNG e JPEG: GIF, BMP e WebP viram PNG.
fn para_png_ou_jpeg(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if matches!(tipo(&bytes), Some("png" | "jpeg")) {
        return Ok(bytes);
    }
    let img = image::load_from_memory(&bytes).map_err(|e| trf!("Imagem corrompida: {e}"))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    imagens_importadas::rgba_para_png(w, h, rgba.into_raw())
}

/// A célula debaixo do canto `(x, y)` da imagem solta (régua das imagens).
fn celula_em(sheet: &feathersheet_core::planilha::sheet::Sheet, x: f64, y: f64) -> (u32, u32) {
    let (r, c, _, _) = super::desenhar::celula_em(sheet,
        (x + imagens_importadas::DESLOC_X) as f32, (y + imagens_importadas::DESLOC_Y) as f32);
    (r, c)
}

impl App {
    /// A célula ativa guarda uma imagem (e não é fórmula).
    pub(super) fn celula_tem_imagem(&self) -> bool {
        self.workbook.aba(self.folha)
            .is_some_and(|a| eh_imagem_na_celula(&a.sheet.get_raw(self.linha, self.coluna)))
    }

    pub(super) fn evento_imagem_celula(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Inserir => {
                self.contexto = None;
                return Task::perform(async {
                    let Some(arquivo) = rfd::AsyncFileDialog::new().set_directory(dialogos::pasta_padrao())
                        .add_filter(tr!("Imagens"), &["png", "jpg", "jpeg", "gif", "bmp", "webp"])
                        .pick_file().await else { return Ok(None); };
                    let caminho = arquivo.path().to_path_buf();
                    tokio::task::spawn_blocking(move || {
                        let nome = caminho.file_name().and_then(|s| s.to_str())
                            .unwrap_or(tr!("Imagem")).to_owned();
                        let tamanho = std::fs::metadata(&caminho)
                            .map_err(|e| trf!("Não consegui acessar a imagem: {e}"))?.len();
                        if tamanho == 0 || tamanho > imagens_importadas::MAX_BYTES as u64 {
                            return Err(tr!("Escolha uma imagem de até 16 MiB.").to_string());
                        }
                        let bytes = std::fs::read(&caminho)
                            .map_err(|e| trf!("Falha ao ler imagem: {e}"))?;
                        Ok(Some((bytes, nome)))
                    }).await.unwrap_or_else(|e| Err(trf!("Falha ao carregar imagem: {e}")))
                }, |r| Msg::ImagemCelula(Evento::Carregada(r)));
            }
            Evento::Carregada(Ok(Some((bytes, nome)))) => {
                if let Err(e) = self.inserir_na_celula(&bytes, &nome) { self.status = e; }
            }
            Evento::Carregada(Ok(None)) => self.status = tr!("Seleção de imagem cancelada.").into(),
            Evento::Carregada(Err(e)) => self.status = e,
            Evento::ColocarNaCelula => {
                self.contexto = None;
                self.imagem_contexto = None;
                if let Err(e) = self.colocar_na_celula_em(None) { self.status = e; }
            }
            Evento::Contexto(id) => {
                if let Some(aba) = self.workbook.aba(self.folha) {
                    self.imagem_selecionada = Some((aba.id, id));
                }
                self.contexto = None;
                self.imagem_contexto = Some((self.ponteiro_x, self.ponteiro_y));
            }
            Evento::FecharContexto => self.imagem_contexto = None,
            Evento::PassouNoMenu => {}
            Evento::Excluir => {
                self.imagem_contexto = None;
                self.apagar_imagem_selecionada();
            }
            Evento::ColocarSobre => {
                self.contexto = None;
                if let Err(e) = self.colocar_sobre() { self.status = e; }
            }
        }
        Task::none()
    }

    fn checar_celula_livre(&self, r: u32, c: u32) -> Result<(), String> {
        let aba = self.workbook.aba(self.folha).ok_or(tr!("A aba selecionada não existe."))?;
        if aba.sheet.celula_bloqueada(r, c) {
            return Err(tr!("A célula está protegida.").into());
        }
        Ok(())
    }

    fn inserir_na_celula(&mut self, bytes: &[u8], nome: &str) -> Result<(), String> {
        self.confirmar();
        let tipo = tipo(bytes).ok_or(tr!("Escolha uma imagem PNG, JPEG, GIF, BMP ou WebP."))?;
        image::load_from_memory(bytes).map_err(|e| trf!("Imagem corrompida: {e}"))?;
        let (r, c) = (self.linha, self.coluna);
        self.checar_celula_livre(r, c)?;
        self.workbook.escrever_celula(self.folha, r, c,
            format!("data:image/{tipo};base64,{}", STANDARD.encode(bytes)));
        self.edicao.clear();
        self.alterado = true;
        self.status = trf!("Imagem {} colocada na célula {}.", nome, cell_name(r, c));
        Ok(())
    }

    /// A imagem solta selecionada vai para a célula debaixo do canto dela
    /// (ou de `canto`, onde o Alt + arrastar a soltou).
    pub(super) fn colocar_na_celula_em(&mut self, canto: Option<(f64, f64)>) -> Result<(), String> {
        self.confirmar();
        let (aba_id, id) = self.imagem_selecionada.clone()
            .ok_or(tr!("Selecione uma imagem solta para colocar na célula."))?;
        let aba = self.workbook.aba(self.folha).filter(|a| a.id == aba_id)
            .ok_or(tr!("A imagem selecionada não existe mais."))?;
        let (src, x, y) = imagens_importadas::src_e_posicao(&aba.sheet, &id)?;
        let (x, y) = canto.unwrap_or((x, y));
        let bytes = bytes_do_texto(&src).ok_or(tr!("Não consegui ler a imagem."))?;
        let tipo = tipo(&bytes).ok_or(tr!("Não consegui ler a imagem."))?;
        let (r, c) = celula_em(&aba.sheet, x, y);
        self.checar_celula_livre(r, c)?;
        let aba = self.workbook.aba_mut(self.folha).ok_or(tr!("A aba selecionada não existe."))?;
        // Tirar a imagem solta e escrever a célula: um passo de desfazer.
        aba.sheet.abrir_transacao();
        let removida = imagens_importadas::remover_imagem(&mut aba.sheet, &id);
        if let Err(e) = removida {
            aba.sheet.reverter_transacao();
            return Err(e);
        }
        self.workbook.escrever_celula(self.folha, r, c,
            format!("data:image/{tipo};base64,{}", STANDARD.encode(&bytes)));
        if let Some(aba) = self.workbook.aba_mut(self.folha) { aba.sheet.fechar_transacao(); }
        self.imagem_selecionada = None;
        self.arraste_imagem = None;
        self.imagens_importadas.handles.borrow_mut().remove(&id);
        self.invalidar_cache_objetos();
        self.trocar_selecao(r, c);
        self.alterado = true;
        self.status = trf!("Imagem colocada na célula {} (Ctrl+Z desfaz).", cell_name(r, c));
        Ok(())
    }

    fn colocar_sobre(&mut self) -> Result<(), String> {
        self.confirmar();
        let (r, c) = (self.linha, self.coluna);
        self.checar_celula_livre(r, c)?;
        let aba = self.workbook.aba(self.folha).ok_or(tr!("A aba selecionada não existe."))?;
        let raw = aba.sheet.get_raw(r, c);
        if !eh_imagem_na_celula(&raw) {
            return Err(tr!("A célula ativa não tem imagem.").into());
        }
        let bytes = para_png_ou_jpeg(bytes_do_texto(&raw).ok_or(tr!("Não consegui ler a imagem."))?)?;
        let nome = format!("{} {}", tr!("Imagem"), cell_name(r, c));
        let aba = self.workbook.aba_mut(self.folha).ok_or(tr!("A aba selecionada não existe."))?;
        // A imagem solta nasce no canto da célula; a célula fica vazia. Um
        // passo de desfazer para as duas coisas.
        let antes = aba.sheet.im.clone();
        let id = imagens_importadas::importar(&mut aba.sheet, &bytes, &nome, r, c)?;
        let novo = std::mem::replace(&mut aba.sheet.im, antes);
        aba.sheet.abrir_transacao();
        aba.sheet.alterar_imagens(novo);
        self.workbook.escrever_celula(self.folha, r, c, String::new());
        if let Some(aba) = self.workbook.aba_mut(self.folha) { aba.sheet.fechar_transacao(); }
        self.edicao.clear();
        self.invalidar_cache_objetos();
        self.imagem_selecionada = self.workbook.aba(self.folha).map(|a| (a.id, id.clone()));
        self.arraste_imagem = None;
        if let Some(imagem) = crate::compat::imagem_de_bytes(&bytes) {
            self.imagens_importadas.handles.borrow_mut().insert(id, imagem);
        }
        self.alterado = true;
        self.status = trf!("Imagem de {} colocada sobre as células (Ctrl+Z desfaz).", cell_name(r, c));
        Ok(())
    }
}

impl App {
    /// O menu do botão direito na imagem solta.
    pub(super) fn sobrepor_menu_imagem<'a>(&'a self, base: pelicano::Element<'a, Msg>) -> pelicano::Element<'a, Msg> {
        use super::menu_contexto::{self, ItemMenu};
        let (Some((x, y)), Some(_)) = (self.imagem_contexto, &self.imagem_selecionada) else { return base };
        let m = Msg::ImagemCelula;
        let itens = vec![
            ItemMenu::acao(tr!("Formatar imagem…"), "palette",
                Msg::FormatarImagem(super::formatar_imagem::Evento::Abrir)),
            ItemMenu::Separador,
            ItemMenu::acao(tr!("Colocar na célula"), "layout-grid", m(Evento::ColocarNaCelula))
                .com_atalho(tr!("Alt+arrastar")),
            ItemMenu::Separador,
            ItemMenu::acao(tr!("Excluir"), "trash", m(Evento::Excluir)).com_atalho(tr!("Delete")),
        ];
        menu_contexto::sobrepor(base, &itens, None, |_| m(Evento::PassouNoMenu),
            m(Evento::FecharContexto), (x, y), self.tamanho_principal)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    // PNG 1×1.
    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

    #[test]
    fn botao_direito_na_imagem_dentro_da_selecao_oferece_colocar_sobre() {
        use base64::Engine as _;
        use crate::app::Contexto;
        let mut app = App::default();
        app.trocar_selecao(2, 1);
        let bytes = base64::engine::general_purpose::STANDARD.decode(PNG).unwrap();
        let _ = app.evento_imagem_celula(Evento::Carregada(Ok(Some((bytes, "x.png".into())))));
        // Seleção A1:D5 com a ativa em A1; a imagem está em B3, dentro dela.
        app.trocar_selecao(0, 0);
        app.fim_selecao = (4, 3);
        let _ = app.update(Msg::AbrirContexto(Contexto::Celula(2, 1), 40.0, 40.0));
        assert_eq!((app.linha, app.coluna), (2, 1));
        assert!(app.opcoes_contexto(Contexto::Celula(2, 1)).iter().flatten()
            .any(|o| o.0 == tr!("Colocar imagem sobre as células")));
    }

    #[test]
    fn colocar_sobre_e_de_volta_na_celula_com_um_ctrl_z_cada() {
        let mut app = App::default();
        let url = format!("data:image/png;base64,{PNG}");
        app.workbook.escrever_celula(0, 2, 1, url.clone());
        app.trocar_selecao(2, 1);
        assert!(app.celula_tem_imagem());

        app.colocar_sobre().unwrap();
        let aba = app.workbook.aba(0).unwrap();
        assert_eq!(aba.sheet.get_raw(2, 1), "", "a célula fica vazia");
        let id = app.imagem_selecionada.clone().unwrap().1;
        let (_, x, y) = imagens_importadas::src_e_posicao(&aba.sheet, &id).unwrap();
        assert_eq!(celula_em(&aba.sheet, x, y), (2, 1), "a imagem solta nasce na própria célula");

        // Ctrl+Z desfaz as duas coisas de uma vez.
        app.workbook.aba_mut(0).unwrap().sheet.undo();
        let aba = app.workbook.aba(0).unwrap();
        assert_eq!(aba.sheet.get_raw(2, 1), url);
        assert!(aba.sheet.im.is_none());
        app.workbook.aba_mut(0).unwrap().sheet.redo();

        app.trocar_selecao(0, 0);
        app.colocar_na_celula_em(None).unwrap();
        let aba = app.workbook.aba(0).unwrap();
        assert_eq!(aba.sheet.get_raw(2, 1), url, "volta para a célula debaixo do canto");
        assert!(aba.sheet.im.is_none(), "e deixa de ser imagem solta");
    }

    #[test]
    fn barra_de_formulas_nao_mostra_nem_apaga_o_base64() {
        let mut app = App::default();
        let url = format!("data:image/png;base64,{PNG}");
        app.workbook.escrever_celula(0, 0, 0, url.clone());
        app.trocar_selecao(0, 0);
        assert_eq!(app.edicao, "");
        assert_eq!(app.formula_para_gravar("", &url), url);
    }

    #[test]
    fn alt_arrastar_solta_a_imagem_dentro_da_celula() {
        use pelicano::Point;
        let mut app = App::default();
        let png = STANDARD.decode(PNG).unwrap();
        app.update(Msg::ImagemLaboratorioCarregada(Ok(Some((png, "x.png".into())))));
        let id = app.imagem_selecionada.clone().unwrap().1;
        app.update(Msg::Grafico(super::super::graficos::Evento::Cursor(Point::new(150., 120.))));
        app.update(Msg::ImagemPressionar(id, imagens_importadas::Gesto::Mover));
        app.imagens_importadas.alt.set(true);
        // 3 colunas para a direita e 4 linhas para baixo, mais ou menos.
        app.update(Msg::Grafico(super::super::graficos::Evento::Cursor(Point::new(150. + 300., 120. + 90.))));
        app.update(Msg::Grafico(super::super::graficos::Evento::Soltar));
        let aba = app.workbook.aba(0).unwrap();
        assert!(aba.sheet.im.is_none(), "deixa de ser imagem solta");
        let (r, c) = (app.linha, app.coluna);
        assert!(eh_imagem_na_celula(&aba.sheet.get_raw(r, c)), "foi para {}", cell_name(r, c));
        assert!(r > 0 && c > 0);
    }
}
