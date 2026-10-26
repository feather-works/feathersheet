//! Lista suspensa da célula ativa, no jeito do Excel: abre logo abaixo da
//! célula (ou acima, sem espaço), mostra até 8 itens e rola com a roda; o
//! item sob o mouse e o escolhido pelo teclado ficam destacados.
//!
//! Desenhada por [`CamadaLista`], um segundo Canvas empilhado sobre a grade:
//! no Iced o texto de um Canvas fica sempre acima dos retângulos dele, e o
//! texto das células atravessava a lista. A camada só desenha; cliques, roda
//! e teclado continuam com a grade e o app.
use super::*;

/// Altura de um item, em pixels lógicos.
pub(crate) const ITEM: f32 = 22.0;
/// Itens à vista de uma vez; o resto rola.
pub(crate) const VISIVEIS: usize = 8;

/// Primeiro item à vista para `inicio` pedido e `total` itens.
pub(crate) fn inicio_valido(inicio: usize, total: usize) -> usize {
    inicio.min(total.saturating_sub(VISIVEIS))
}

/// Primeiro item à vista que mantém `destaque` dentro da lista.
pub(crate) fn rolar_ate(inicio: usize, destaque: usize, total: usize) -> usize {
    let inicio = if destaque < inicio { destaque }
        else if destaque >= inicio + VISIVEIS { destaque + 1 - VISIVEIS }
        else { inicio };
    inicio_valido(inicio, total)
}

/// As opções da validação de lista na célula (`r`, `c`), sem repetidas.
pub(crate) fn opcoes_validacao(workbook: &feathersheet_core::planilha::workbook::Workbook, aba: usize, r: u32, c: u32) -> Option<Vec<String>> {
    use feathersheet_core::operacoes::validation::Validacao;
    let folha = workbook.aba(aba)?;
    let regras: Vec<Validacao> = serde_json::from_str(folha.sheet.va.as_ref()?).ok()?;
    let regra = regras.iter().rev().find(|v| v.ranges.iter().any(|range| range.contains(r, c)))?;
    let mut itens = workbook.opcoes_da_lista(aba, r, c, &regra.regra)?;
    let mut vistos = std::collections::HashSet::new();
    itens.retain(|s| vistos.insert(s.clone()));
    Some(itens)
}

/// Canvas da lista aberta, empilhado sobre o da grade com o mesmo tamanho.
pub(crate) struct CamadaLista<'a>(pub Grade<'a>);

impl canvas::Program<Msg> for CamadaLista<'_> {
    type State = ();

    fn draw(&self, _state: &(), renderer: &mut Renderer, bounds: Rectangle, _theme: &Theme,
        cursor: Option<Point>)
    {
        // `bounds` na janela: cada quadro abre o recorte nele e põe a origem no canto.
        self.0.desenhar_baloes(renderer, bounds);
        self.0.desenhar_lista(renderer, bounds, cursor);
    }
    /// O da grade de baixo (a camada só desenha).
    fn mouse_interaction(&self, _state: &(), _bounds: Rectangle, _cursor: Option<Point>) -> mouse::Interaction {
        self.0.cache.cursor.get()
    }
}

impl Grade<'_> {
    /// Balões dos comentários e da mensagem de entrada da validação. Ficam
    /// nesta camada pelo mesmo motivo da lista: no canvas da grade o texto
    /// das células atravessava o fundo deles.
    fn desenhar_baloes(&self, renderer: &mut Renderer, tela: Rectangle) {
        let tem_comentario = self.workbook.aba(self.aba).is_some_and(|a| !a.sheet.comentarios.is_empty());
        let (r, c) = self.selecionada;
        let tem_mensagem = !self.lista_aberta
            && super::validacao_visual::mensagem_de_entrada(self.workbook, self.aba, r, c).is_some();
        if !tem_comentario && !tem_mensagem { return; }
        let area = Size::new(tela.width / self.zoom, tela.height / self.zoom);
        let (colunas, _) = self.colunas_visiveis(area.width);
        let (linhas, _) = self.linhas_visiveis(area.height);
        let mut f = Frame::new(renderer, tela);
        f.scale(self.zoom);
        if tem_comentario { self.desenhar_comentarios(&mut f, &colunas, &linhas, area, true); }
        if tem_mensagem { self.desenhar_validacao(&mut f, &colunas, &linhas, area, true); }
        f.fim();
    }
}

impl Grade<'_> {
    /// Retângulo da lista (x, y, largura, altura) e quantos itens mostra.
    pub(super) fn lista_popup(&self, viewport: Size, quantidade: usize)
        -> Option<(f32, f32, f32, f32, usize)>
    {
        let (x, y, w, h) = self.retangulo_celula_ativa(viewport)?;
        let linhas = quantidade.clamp(1, VISIVEIS);
        let altura = linhas as f32 * ITEM + 2.0;
        // Perto da borda direita, a lista vira para a esquerda em vez de cortar.
        let largura = w.max(160.0).min(320.0)
            .min((viewport.width - CABECALHO_LINHA - 4.0).max(0.0));
        if largura < 80.0 { return None; }
        let esquerda = x.min((viewport.width - largura - 3.0).max(CABECALHO_LINHA));
        let topo = if y + h + altura <= viewport.height { y + h }
            else { (y - altura).max(CABECALHO_COLUNA) };
        Some((esquerda, topo, largura, altura, linhas))
    }

    /// Índice do item sob o ponto (lógico), se o ponto cair na lista.
    pub(super) fn item_da_lista(&self, p: Point, viewport: Size, total: usize) -> Option<Option<usize>> {
        let (px, py, pw, ph, linhas) = self.lista_popup(viewport, total)?;
        if p.x < px || p.x >= px + pw || p.y < py || p.y >= py + ph { return None; }
        let linha = ((p.y - py - 1.0) / ITEM).max(0.0) as usize;
        let indice = inicio_valido(self.lista_pagina, total) + linha;
        Some((linha < linhas && indice < total).then_some(indice))
    }

    /// A camada da lista aberta, por cima de tudo o que a grade desenha.
    pub(super) fn desenhar_lista(&self, renderer: &mut Renderer, tela: Rectangle, cursor: Option<Point>) {
        if !self.lista_aberta { return; }
        let Some(opcoes) = self.opcoes_lista() else { return };
        let viewport = Size::new(tela.width / self.zoom, tela.height / self.zoom);
        let Some((px, py, pw, ph, linhas)) = self.lista_popup(viewport, opcoes.len()) else { return };
        let mut f = Frame::new(renderer, tela);
        f.scale(self.zoom);
        let (acento, fundo, texto, borda, hover, marcado) = if self.escuro {
            (Color::rgb8(122, 180, 248), Color::rgb8(35, 40, 49), Color::rgb8(239, 242, 248),
             Color::rgb8(86, 96, 112), Color::rgb8(52, 61, 74), Color::rgb8(44, 78, 128))
        } else {
            (Color::rgb8(35, 112, 204), Color::WHITE, Color::rgb8(40, 48, 60),
             Color::rgb8(160, 170, 184), Color::rgb8(235, 242, 252), Color::rgb8(204, 224, 250))
        };
        // Sombra leve, borda e fundo.
        f.fill_rectangle(Point::new(px + 2.0, py + 3.0), Size::new(pw, ph), Color::rgb8(0, 0, 0).alpha(0.14));
        f.fill_rectangle(Point::new(px - 1.0, py - 1.0), Size::new(pw + 2.0, ph + 2.0), borda);
        f.fill_rectangle(Point::new(px, py), Size::new(pw, ph), fundo);
        let sob_mouse = local(cursor, tela)
            .map(|p| Point::new(p.x / self.zoom, p.y / self.zoom))
            .and_then(|p| self.item_da_lista(p, viewport, opcoes.len()).flatten());
        let atual = self.workbook.aba(self.aba)
            .map(|a| a.sheet.get_raw(self.selecionada.0, self.selecionada.1))
            .unwrap_or_default();
        let rola = opcoes.len() > VISIVEIS;
        let largura_texto = pw - if rola { 26.0 } else { 18.0 };
        let inicio = inicio_valido(self.lista_pagina, opcoes.len());
        for (linha, item) in opcoes.iter().skip(inicio).take(linhas).enumerate() {
            let indice = inicio + linha;
            let iy = py + 1.0 + linha as f32 * ITEM;
            let cor = if indice == self.lista_destaque { Some(marcado) }
                else if Some(indice) == sob_mouse { Some(hover) } else { None };
            if let Some(cor) = cor {
                f.fill_rectangle(Point::new(px + 1.0, iy), Size::new(pw - 2.0, ITEM), cor);
            }
            let max_chars = (largura_texto / 7.0).floor().max(2.0) as usize;
            let nome = if item.chars().count() > max_chars {
                format!("{}…", item.chars().take(max_chars - 1).collect::<String>())
            } else { item.clone() };
            f.fill_text(Text {
                content: nome, position: Point::new(px + 8.0, iy + ITEM / 2.0),
                color: if *item == atual { acento } else { texto }, size: crate::compat::Pixels(12.5),
                vertical_alignment: pelicano::Align::Center,
                ..Text::default()
            });
        }
        if rola {
            // Barra de rolagem fina, proporcional à parte à vista.
            let trilho = ph - 4.0;
            let altura = (trilho * VISIVEIS as f32 / opcoes.len() as f32).max(14.0);
            let maximo = opcoes.len() - VISIVEIS;
            let y = py + 2.0 + (trilho - altura) * inicio as f32 / maximo as f32;
            f.fill_rectangle(Point::new(px + pw - 7.0, y), Size::new(4.0, altura), borda);
        }
        f.fim();
    }
}
