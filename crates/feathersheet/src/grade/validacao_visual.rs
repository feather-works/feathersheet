//! Validação de dados na grade: a mensagem de entrada (balão ao selecionar a
//! célula, como no Excel) e os círculos vermelhos de "Circular dados
//! inválidos". Só desenha o que está no viewport; as regras vêm do motor
//! (`Validacao::mensagem_de_entrada`, `Workbook::validacao_da_celula`).
use super::Grade;
use super::comentarios::quebrar;
use super::congelado::Faixa;
use feather_ui::tema::{COR_AZUL_ACENTO, COR_BORDA, COR_PAINEL, COR_TEXTO};
use feathersheet_core::operacoes::validation::Validacao;
use feathersheet_core::planilha::workbook::Workbook;
use pelicano::Color; use pelicano::Point; use pelicano::Size; use crate::compat::Vector; use crate::compat::canvas; use crate::compat::canvas::path::arc; use crate::compat::canvas::Frame; use crate::compat::canvas::Text;

/// Vermelho dos círculos (o mesmo do indicador de comentário).
const COR_CIRCULO: Color = Color::rgb(220.0 / 255.0, 38.0 / 255.0, 38.0 / 255.0);
const LARGURA_BALAO: f32 = 220.0;
const TAMANHO_TEXTO: f32 = 12.0;
const ALTURA_LINHA: f32 = 16.0;
const CARACTERES_POR_LINHA: usize = 30;
const MAX_LINHAS: usize = 10;

/// `(título, texto)` da mensagem de entrada da célula, se a regra tiver.
pub(crate) fn mensagem_de_entrada(workbook: &Workbook, aba: usize, r: u32, c: u32) -> Option<(String, String)> {
    let json = workbook.aba(aba)?.sheet.va.as_deref()?;
    let regras: Vec<Validacao> = serde_json::from_str(json).ok()?;
    Workbook::validacao_da_celula(&regras, r, c)?.mensagem_de_entrada()
}

impl Grade<'_> {
    /// Círculos de dados inválidos (`baloes` falso, na grade) ou o balão da
    /// mensagem de entrada (`baloes` verdadeiro, na camada de cima).
    pub(super) fn desenhar_validacao(&self, f: &mut Frame, colunas: &[Faixa], linhas: &[Faixa], area: Size, baloes: bool) {
        let celula = |r: u32, c: u32| {
            let &(_, x, w) = colunas.iter().find(|&&(cc, _, _)| cc == c)?;
            let &(_, y, h) = linhas.iter().find(|&&(rr, _, _)| rr == r)?;
            Some((x, y, w, h))
        };
        // Círculos: elipse um pouco maior que a célula, traço de 2 px.
        for &(r, c) in self.circulos_validacao.iter().filter(|_| !baloes) {
            let Some((x, y, w, h)) = celula(r, c) else { continue };
            let elipse = canvas::Path::new(|p| p.ellipse(arc::Elliptical {
                center: Point::new(x + w / 2.0, y + h / 2.0),
                radii: Vector::new(w / 2.0 + 5.0, h / 2.0 + 4.0),
            }));
            f.stroke(&elipse, canvas::Stroke::default().with_color(COR_CIRCULO).with_width(2.0));
        }
        // Mensagem de entrada da célula ativa (fora da lista suspensa aberta).
        if !baloes || self.lista_aberta { return; }
        let (r, c) = self.selecionada;
        let Some((x, y, w, h)) = celula(r, c) else { return };
        let Some((titulo, texto)) = mensagem_de_entrada(self.workbook, self.aba, r, c) else { return };
        let linhas_texto = if texto.is_empty() { Vec::new() } else { quebrar(&texto, CARACTERES_POR_LINHA, MAX_LINHAS) };
        let com_titulo = !titulo.is_empty();
        let altura = 12.0 + ALTURA_LINHA * (linhas_texto.len() + usize::from(com_titulo)) as f32;
        // À direita da célula, na altura do meio dela (não cobre a coluna,
        // onde ficam os círculos); sem espaço, vai para a esquerda.
        let mut bx = x + w + 10.0;
        if bx + LARGURA_BALAO > area.width - 4.0 { bx = (x - LARGURA_BALAO - 10.0).max(super::CABECALHO_LINHA + 2.0); }
        let by = (y + h / 2.0 - 8.0).min(area.height - altura - 4.0).max(super::CABECALHO_COLUNA + 2.0);
        let caixa = canvas::Path::rounded_rectangle(Point::new(bx, by), Size::new(LARGURA_BALAO, altura), 6.0);
        f.fill(&caixa, COR_PAINEL());
        f.stroke(&caixa, canvas::Stroke::default().with_color(COR_BORDA()).with_width(1.0));
        f.fill_rectangle(Point::new(bx, by + 6.0), Size::new(3.0, altura - 12.0), COR_AZUL_ACENTO());
        let mut ty = by + 6.0;
        let mut escrever = |conteudo: String, negrito: bool, ty: f32| f.fill_text(Text {
            content: conteudo,
            position: Point::new(bx + 10.0, ty),
            color: COR_TEXTO(),
            size: crate::compat::Pixels(TAMANHO_TEXTO),
            font: if negrito { pelicano::Font { weight: 700, ..pelicano::Font::DEFAULT } } else { pelicano::Font::DEFAULT },
            horizontal_alignment: pelicano::Align::Start,
            vertical_alignment: pelicano::Align::Start,
            ..Text::default()
        });
        if com_titulo {
            let t: String = titulo.chars().take(CARACTERES_POR_LINHA).collect();
            escrever(t, true, ty);
            ty += ALTURA_LINHA;
        }
        for linha in linhas_texto {
            escrever(linha, false, ty);
            ty += ALTURA_LINHA;
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::refs::Rect;
    use feathersheet_core::operacoes::validation::{MensagensValidacao, QuandoValidacao, Regra};

    #[test]
    fn mensagem_de_entrada_vem_da_regra_da_celula() {
        let mut wb = Workbook::new();
        let regra = Validacao {
            ranges: vec![Rect::new(0, 0, 4, 0)],
            regra: Regra::Lista { valores: vec!["Sim".into()] },
            quando: QuandoValidacao::Recusar, mensagem: None,
            mensagens: MensagensValidacao { entrada_titulo: Some("Status".into()),
                entrada_texto: Some("Escolha da lista".into()), erro_titulo: None },
        };
        wb.editar(0, |s| s.va = Some(serde_json::to_string(&vec![regra]).unwrap()));
        assert_eq!(mensagem_de_entrada(&wb, 0, 3, 0), Some(("Status".into(), "Escolha da lista".into())));
        assert_eq!(mensagem_de_entrada(&wb, 0, 3, 1), None);
    }
}
