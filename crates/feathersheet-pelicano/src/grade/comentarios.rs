//! Comentários na grade: o triângulo no canto superior direito da célula
//! comentada e o balão (ao passar o mouse, sempre à mostra ou "Mostrar
//! todos"). Só desenha o que está no viewport; o texto vem do motor
//! (`Sheet::comentarios`), nada é copiado para a interface.
use super::Grade;
use super::congelado::Faixa;
use feather_ui::tema::{COR_BORDA, COR_PAINEL, COR_TEXTO, COR_TEXTO_MUTED};
use pelicano::Color; use pelicano::Point; use pelicano::Size; use crate::compat::canvas; use crate::compat::canvas::Frame; use crate::compat::canvas::Text;
use std::collections::HashMap;

/// O que a interface quer ver dos comentários (não é dado do documento).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VisaoComentarios {
    /// Célula com comentário sob o mouse (ou escolhida por Anterior/Próximo).
    pub sobre: Option<(u32, u32)>,
    /// Revisão > Mostrar todos os comentários.
    pub todos: bool,
    /// Célula cujo comentário está aberto no editor: o balão dela sai.
    pub editando: Option<(u32, u32)>,
}

/// Indicador do Excel: vermelho, cor de conteúdo da grade (a mesma da
/// seta de rastreio de erro), legível nos dois temas.
const COR_INDICADOR: Color = Color::rgb(220.0 / 255.0, 38.0 / 255.0, 38.0 / 255.0);
const LADO_TRIANGULO: f32 = 7.0;
const LARGURA_BALAO: f32 = 210.0;
const TAMANHO_TEXTO: f32 = 12.0;
const ALTURA_LINHA: f32 = 16.0;
/// ~6,6 px por caractere em 12 pt: quebra manual (Canvas não quebra linha).
const CARACTERES_POR_LINHA: usize = 29;
const MAX_LINHAS: usize = 12;

/// Quebra o texto em linhas de até `largura` caracteres, por palavra,
/// respeitando as quebras do próprio texto; corta com `…` depois de `max`.
pub(crate) fn quebrar(texto: &str, largura: usize, max: usize) -> Vec<String> {
    let mut linhas: Vec<String> = Vec::new();
    for paragrafo in texto.split('\n') {
        let mut atual = String::new();
        for palavra in paragrafo.split(' ') {
            let mut palavra = palavra.to_string();
            // Palavra maior que a linha: parte no meio.
            while palavra.chars().count() > largura {
                if !atual.is_empty() { linhas.push(std::mem::take(&mut atual)); }
                let corte: String = palavra.chars().take(largura).collect();
                palavra = palavra.chars().skip(largura).collect();
                linhas.push(corte);
            }
            let cabe = atual.is_empty() || atual.chars().count() + 1 + palavra.chars().count() <= largura;
            if !cabe { linhas.push(std::mem::take(&mut atual)); }
            if !atual.is_empty() { atual.push(' '); }
            atual.push_str(&palavra);
        }
        linhas.push(atual);
    }
    if linhas.len() > max {
        linhas.truncate(max);
        if let Some(ultima) = linhas.last_mut() {
            let mut t: String = ultima.chars().take(largura.saturating_sub(1)).collect();
            t.push('…');
            *ultima = t;
        }
    }
    linhas
}

impl Grade<'_> {
    /// Triângulos (`baloes` falso, na grade) ou balões (`baloes` verdadeiro,
    /// na camada de cima) dos comentários visíveis. `colunas`/`linhas` são
    /// as faixas já calculadas para este quadro; `area` é o viewport (sem zoom).
    pub(super) fn desenhar_comentarios(&self, f: &mut Frame, colunas: &[Faixa], linhas: &[Faixa], area: Size, baloes: bool) {
        let Some(aba) = self.workbook.aba(self.aba) else { return };
        let notas = &aba.sheet.comentarios;
        if notas.is_empty() { return; }
        let xs: HashMap<u32, (f32, f32)> = colunas.iter().map(|&(c, x, w)| (c, (x, w))).collect();
        let mut balloes = Vec::new();
        for &(r, y, h) in linhas {
            for (&(_, c), nota) in notas.range((r, 0)..=(r, u32::MAX)) {
                let Some(&(x, w)) = xs.get(&c) else { continue };
                if w < 4.0 || h < 4.0 { continue; }
                let lado = LADO_TRIANGULO.min(w / 2.0).min(h / 2.0);
                let tri = canvas::Path::new(|p| {
                    p.move_to(Point::new(x + w - lado, y));
                    p.line_to(Point::new(x + w, y));
                    p.line_to(Point::new(x + w, y + lado));
                    p.close();
                });
                if !baloes { f.fill(&tri, COR_INDICADOR); continue; }
                let mostrar = (nota.visivel || self.comentarios.todos || self.comentarios.sobre == Some((r, c)))
                    && self.comentarios.editando != Some((r, c));
                if mostrar { balloes.push((x + w, y, nota)); }
            }
        }
        // Balões por cima de tudo, na ordem da aba.
        for (canto_x, canto_y, nota) in balloes {
            let linhas_texto = quebrar(&nota.texto, CARACTERES_POR_LINHA, MAX_LINHAS);
            let com_autor = !nota.autor.is_empty();
            let altura = 12.0 + ALTURA_LINHA * (linhas_texto.len() + usize::from(com_autor)) as f32;
            // À direita da célula; sem espaço, vai para a esquerda dela.
            let mut x = canto_x + 12.0;
            if x + LARGURA_BALAO > area.width - 4.0 { x = (canto_x - LARGURA_BALAO - 24.0).max(super::CABECALHO_LINHA + 2.0); }
            let y = (canto_y - 2.0).min(area.height - altura - 4.0).max(super::CABECALHO_COLUNA + 2.0);
            // Traço do canto da célula até o balão, como no Excel.
            let ligacao = canvas::Path::line(Point::new(canto_x, canto_y), Point::new(x, y + 8.0));
            f.stroke(&ligacao, canvas::Stroke::default().with_color(COR_TEXTO_MUTED()).with_width(1.0));
            let caixa = canvas::Path::rounded_rectangle(Point::new(x, y), Size::new(LARGURA_BALAO, altura), 6.0);
            f.fill(&caixa, COR_PAINEL());
            f.stroke(&caixa, canvas::Stroke::default().with_color(COR_BORDA()).with_width(1.0));
            f.fill_rectangle(Point::new(x, y + 6.0), Size::new(3.0, altura - 12.0), COR_INDICADOR);
            let mut ty = y + 6.0;
            if com_autor {
                f.fill_text(Text {
                    content: format!("{}:", nota.autor),
                    position: Point::new(x + 10.0, ty),
                    color: COR_TEXTO(),
                    size: crate::compat::Pixels(TAMANHO_TEXTO),
                    font: pelicano::Font { weight: 700, ..pelicano::Font::DEFAULT },
                    horizontal_alignment: pelicano::Align::Start,
                    vertical_alignment: pelicano::Align::Start,
                    ..Text::default()
                });
                ty += ALTURA_LINHA;
            }
            for linha in linhas_texto {
                f.fill_text(Text {
                    content: linha,
                    position: Point::new(x + 10.0, ty),
                    color: COR_TEXTO(),
                    size: crate::compat::Pixels(TAMANHO_TEXTO),
                    horizontal_alignment: pelicano::Align::Start,
                    vertical_alignment: pelicano::Align::Start,
                    ..Text::default()
                });
                ty += ALTURA_LINHA;
            }
        }
    }

    /// Célula com comentário sob o ponto (coordenadas do Canvas, com zoom).
    pub(super) fn comentario_no_ponto(&self, p: Point, tamanho: Size) -> Option<(u32, u32)> {
        let aba = self.workbook.aba(self.aba)?;
        if aba.sheet.comentarios.is_empty() { return None; }
        let (r, c) = self.celula_no_ponto(p, tamanho)?;
        aba.sheet.comentario(r, c).map(|_| (r, c))
    }
}

#[cfg(test)]
mod testes {
    use super::quebrar;

    #[test]
    fn quebra_por_palavra_e_corta_com_reticencias() {
        assert_eq!(quebrar("um dois tres", 7, 5), vec!["um dois", "tres"]);
        assert_eq!(quebrar("a\nb", 10, 5), vec!["a", "b"]);
        assert_eq!(quebrar("abcdefghij", 4, 5), vec!["abcd", "efgh", "ij"]);
        let l = quebrar("1 2 3 4 5 6", 1, 3);
        assert_eq!(l.len(), 3);
        assert!(l[2].ends_with('…'));
    }
}
