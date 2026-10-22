//! Laboratório PNG/JPEG nativo do FeatherSheet.
//! Usa exatamente a mesma API de presets e o mesmo painel Iced do FeatherDocs.
//! Não altera células nem grava raster no .fshz: o arquivo processado é exportado
//! explicitamente como PNG até existir contrato de objetos de imagem no Sheet.
use super::Msg;
use feather_ui::efeitos_imagem;
use feather_image::{EfeitoRapido, ImagemRgba};
use pelicano::Element; use pelicano::Length; use pelicano::button; use pelicano::column; use pelicano::container; use pelicano::image as iced_image; use pelicano::Image; use pelicano::row; use pelicano::text;
use image::{DynamicImage, ImageFormat, ImageReader};
use std::io::Cursor;

const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_PIXELS: u64 = 4_000_000;

pub struct Estado {
    pub aberto: bool,
    pub nome: String,
    /// ID estável da imagem em `sheet.im` enquanto o painel estiver aberto.
    pub imagem_id: Option<String>,
    pub selecionado: &'static str,
    original: Option<ImagemRgba>,
    processado: Option<ImagemRgba>,
    previa: Option<Image>,
}

impl Default for Estado {
    fn default() -> Self {
        Self {
            aberto: false,
            nome: String::new(),
            imagem_id: None,
            selecionado: "original",
            original: None,
            processado: None,
            previa: None,
        }
    }
}

impl Estado {
    /// Decodificação protegida por limite de bytes e dimensões, antes de expandir RGBA.
    /// A imagem antiga permanece íntegra se a seleção de arquivo falhar.
    pub fn carregar(&mut self, bytes: &[u8], nome: &str) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > MAX_BYTES {
            return Err(tr!("Imagem vazia ou acima de 16 MiB.").into());
        }
        let formato = image::guess_format(bytes)
            .map_err(|_| tr!("O arquivo não é PNG/JPEG válido.").to_string())?;
        if !matches!(formato, ImageFormat::Png | ImageFormat::Jpeg) {
            return Err(tr!("São aceitas somente imagens PNG e JPEG.").into());
        }
        let (w, h) = ImageReader::with_format(Cursor::new(bytes), formato)
            .into_dimensions()
            .map_err(|e| trf!("Cabeçalho de imagem inválido: {e}"))?;
        if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
            return Err(tr!("A imagem excede 4 milhões de pixels.").into());
        }
        let rgba = image::load_from_memory(bytes)
            .map_err(|e| trf!("Imagem corrompida: {e}"))?.to_rgba8();
        let original = ImagemRgba::de_image(rgba)
            .map_err(|e| trf!("Buffer RGBA inválido: {e:?}"))?;
        let previa = Image::from_rgba(w, h, original.pixels().to_vec());
        self.original = Some(original.clone());
        self.processado = Some(original);
        self.previa = previa;
        self.nome = nome.to_owned();
        self.imagem_id = None;
        self.selecionado = "original";
        self.aberto = true;
        Ok(())
    }

    /// Renderiza sempre a partir da fonte, sem acumular erros ao alternar filtros.
    pub fn aplicar(&mut self, chave: &'static str) -> Result<(), String> {
        let efeito = EfeitoRapido::da_chave(chave)
            .ok_or_else(|| tr!("Efeito de imagem desconhecido.").to_string())?;
        let origem = self.original.as_ref().ok_or(tr!("Abra uma imagem antes de aplicar efeitos."))?;
        let imagem = efeito.executar(origem)
            .map_err(|e| trf!("Efeito inválido: {e:?}"))?;
        let (w, h) = imagem.dimensoes();
        let handle = Image::from_rgba(w, h, imagem.pixels().to_vec());
        self.processado = Some(imagem);
        self.previa = handle;
        self.selecionado = chave;
        Ok(())
    }

    pub fn png_exportacao(&self) -> Result<Vec<u8>, String> {
        let imagem = self.processado.as_ref().ok_or(tr!("Nenhuma imagem processada."))?;
        let mut buffer = Vec::new();
        DynamicImage::ImageRgba8(imagem.clone().em_image())
            .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
            .map_err(|e| trf!("Erro ao codificar PNG: {e}"))?;
        Ok(buffer)
    }

    pub fn painel(&self) -> Element<'_, Msg> {
        let visual: Element<'_, Msg> = if let Some(handle) = &self.previa {
            container(iced_image(handle.clone())
                .width(Length::Fixed(260.0))
                .height(Length::Fixed(180.0)))
                .into()
        } else {
            container(text(tr!("Abra um arquivo PNG/JPEG para visualizar efeitos.")).size(12)).into()
        };
        container(column![
            row![
                text(trf!("Efeitos de imagem · {}", self.nome)).size(13),
                button(tr!("Trocar imagem")).on_press(Msg::ImagemLaboratorioAbrir),
                button(tr!("Exportar PNG")).on_press_maybe(
                    self.processado.is_some().then_some(Msg::ImagemLaboratorioSalvar)
                ),
                button(tr!("Fechar")).on_press(Msg::ImagemLaboratorioFechar),
            ].spacing(10),
            row![
                visual,
                efeitos_imagem::painel(
                    self.selecionado,
                    Msg::ImagemLaboratorioAplicar,
                    Msg::ImagemLaboratorioFechar,
                ),
            ].spacing(12),
            text(tr!("Prévia independente: o .fshz e as células permanecem inalterados.")).size(11),
        ].spacing(8))
        .padding(12)
        .width(Length::FILL)
        .into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use image::{Rgba, RgbaImage};
    fn png() -> Vec<u8> {
        let mut bytes = Vec::new();
        let imagem = RgbaImage::from_fn(3, 2, |x, y| Rgba([
            (x * 65) as u8, (y * 90) as u8, 31, (x * 80 + y * 15) as u8,
        ]));
        DynamicImage::ImageRgba8(imagem).write_to(
            &mut Cursor::new(&mut bytes), ImageFormat::Png
        ).unwrap();
        bytes
    }

    #[test]
    fn img014_sheet_muda_visual_sem_alterar_origem_e_preserva_alpha() {
        let bytes = png();
        let mut estado = Estado::default();
        estado.carregar(&bytes, "teste.png").unwrap();
        let original = estado.original.clone().unwrap();
        estado.aplicar("cinza").unwrap();
        let processado = estado.png_exportacao().unwrap();
        let decodificado = image::load_from_memory(&processado).unwrap().to_rgba8();
        assert_eq!(decodificado.dimensions(), (3, 2));
        assert_eq!(estado.original.as_ref().unwrap(), &original);
        assert_eq!(decodificado.as_raw()[3], original.pixels()[3]);
        assert_eq!(decodificado.as_raw()[0], decodificado.as_raw()[1]);
        estado.aplicar("original").unwrap();
        assert_eq!(estado.processado.as_ref().unwrap(), &original);
    }

    #[test]
    fn img014_sheet_recusa_formatos_invalidos_e_preserva_imagem_anterior() {
        let mut estado = Estado::default();
        estado.carregar(&png(), "ok.png").unwrap();
        assert!(estado.carregar(&[0, 1, 2], "quebrado.png").is_err());
        assert!(estado.aplicar("desconhecido").is_err());
        assert_eq!(estado.nome, "ok.png");
        assert_eq!(estado.selecionado, "original");
        assert!(estado.png_exportacao().is_ok());
        let _: Element<'_, Msg> = estado.painel();
    }
}
