//! Grade desenhada em Canvas (Iced), sem um widget para cada célula.
//! O desenho e o clique são proporcionais ao viewport, nunca aos 100 milhões
//! de linhas da planilha. As fórmulas são avaliadas pelo feathersheet-core.
use feathersheet_core::{
    celulas::{refs::{col_name, MAX_COLS, MAX_ROWS},
        style::{format_value, effective_align, Align}},
};
#[cfg(test)]
use feathersheet_core::celulas::values::Value;
use crate::compat::alignment; use crate::compat::mouse; use pelicano::{Captured, Color, Event, Point, Renderer, Size, Theme}; use crate::compat::Rectangle; use crate::compat::canvas; use crate::compat::canvas::Frame; use crate::compat::canvas::Text;
use crate::app::{Contexto, Msg};
use feathersheet_core::formula::ref_drag::drag_rect;
mod auxiliares;
pub(crate) mod lista_suspensa;
pub(crate) mod modelo;
pub use modelo::{CacheGrade, Grade};
use modelo::{ajustes_texto,largura,altura};
mod congelado;
mod eixos;
pub(crate) use eixos::EstadoEixos;
mod manter_visivel;
pub(crate) use manter_visivel::Exigencia;
pub mod impressao;
pub use impressao::ImpressaoVisual;
mod desenho_auxiliar;
mod mesclas_visuais;
mod referencias;
pub(crate) mod barras;
mod setas;
mod topicos;
mod alca;
pub(crate) mod comentarios;
pub(crate) mod validacao_visual;
pub use comentarios::VisaoComentarios;
use referencias::ReferenciaAtiva;
use desenho_auxiliar::{cor_contraste, cor_hex, desenhar_funil, divisoria_visivel, realce_cruz, RealceCruz};

/// Como o Canvas escala a grade inteira, a largura acompanha o zoom.
/// Cabe "100000000" (nove dígitos, o limite de linhas) com folga.
pub const CABECALHO_LINHA: f32 = 100.0;
pub const CABECALHO_COLUNA: f32 = 30.0;
pub const LARGURA_PADRAO: f32 = 122.0;

/// A largura das colunas sem largura própria nesta aba: a Largura padrão
/// escolhida em Formatar ▾, ou a da grade.
pub fn largura_padrao_da(sheet: &feathersheet_core::planilha::sheet::Sheet) -> f32 {
    sheet.largura_padrao.map(|v| v as f32).unwrap_or(LARGURA_PADRAO)
}
pub const ALTURA_PADRAO: f32 = 28.0;

/// Estado do mouse no Canvas: arrasto de células, arrasto em cabeçalho,
/// modificadores e último clique numa divisória (para o duplo clique).
#[derive(Default)]
pub struct EstadoMouse {
    celulas: bool,
    referencia: Option<ReferenciaAtiva>,
    cabecalho: Option<(bool, u32)>,
    shift: bool,
    divisoria: Option<(std::time::Instant, bool, u32)>,
    ultimo_clique_celula: Option<(std::time::Instant, u32, u32)>,
    /// Arrastando a alça de uma barra de rolagem.
    barra: Option<barras::ArrastoBarra>,
    /// Arrastando a divisória de um cabeçalho para mudar o tamanho.
    dimensao: Option<ArrastoDimensao>,
    /// Arrastando a alça de preenchimento, e o último clique nela (duplo clique).
    alca: Option<alca::ArrastoAlca>,
    ultimo_clique_alca: Option<std::time::Instant>,
    /// Fração da roda ainda não andada (linhas, colunas): o pelicano espalha um
    /// clique da roda por vários quadros, e cada pedaço sozinho arredondaria errado.
    roda: (f32, f32),
}

/// O cursor (na janela) em coordenadas do canvas, se estiver em cima dele
/// (o `local(cursor, bounds)` do Iced).
pub(crate) fn local(cursor: Option<Point>, bounds: Rectangle) -> Option<Point> {
    cursor.filter(|p| bounds.contains(*p)).map(|p| Point::new(p.x - bounds.x, p.y - bounds.y))
}

/// Anda `v` na fração guardada e devolve os passos inteiros já completos.
fn passos_da_roda(resto: &mut f32, v: f32) -> i32 {
    *resto += v;
    let n = resto.round();
    *resto -= n;
    n as i32
}

/// Divisória de cabeçalho sendo arrastada: a linha-guia acompanha o mouse e
/// o tamanho só é gravado ao soltar (um Ctrl+Z), como no Excel.
#[derive(Debug, Clone, Copy)]
struct ArrastoDimensao {
    coluna: bool,
    indice: u32,
    /// Começo da coluna/linha na tela, em unidades da grade (sem zoom).
    inicio: f32,
    /// Posição atual da divisória, nas mesmas unidades.
    atual: f32,
    /// Tamanho antes do arrasto: soltar sem mexer não grava nada.
    original: u32,
}

impl ArrastoDimensao {
    fn tamanho(&self) -> u32 { (self.atual - self.inicio).max(0.0).round() as u32 }
}

/// Folga em pixels lógicos para acertar a divisória entre cabeçalhos.
const FOLGA_DIVISORIA: f32 = 4.0;
const INTERVALO_DUPLO_CLIQUE: std::time::Duration = std::time::Duration::from_millis(450);

/// Recorta o desenho à célula `r` (em unidades da grade, sem zoom).
///
/// O `with_clip` do Iced 0.13 abre um quadro novo que só herda a posição do
/// recorte, não a escala: sem isto o texto ficava em 100% e fora do lugar
/// com qualquer outro zoom. Dentro de `desenhar` a origem é o canto da célula.
fn recortar_celula(f: &mut Frame, zoom: f32, r: Rectangle, desenhar: impl FnOnce(&mut Frame)) {
    let na_tela = Rectangle::new(Point::new(r.x * zoom, r.y * zoom),
        Size::new(r.width * zoom, r.height * zoom));
    f.with_clip(na_tela, |frame| {
        frame.scale(zoom);
        desenhar(frame);
    });
}

impl canvas::Program<Msg> for Grade<'_> {
    type State = EstadoMouse;

    fn draw(
        &self, state: &Self::State, renderer: &mut Renderer, bounds: Rectangle,
        _theme: &Theme, _cursor: Option<Point>,
    ) {
        let inicio_vigia = std::time::Instant::now();
        let tela = bounds;
        // Guarda o tamanho real do Canvas: manter a célula à vista e o Page
        // Down contam com ele (zoom e alturas reais), não com 28 × 12 fixos.
        self.eixos.borrow_mut().tamanho_canvas = Some(tela.size());
        // Sem cache de desenho: o pelicano já só repinta quando algo mudou.
        {
        let mut f = Frame::new(renderer, bounds);
        f.scale(self.zoom);
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(bounds.width / self.zoom,bounds.height / self.zoom));
        // O Canvas lê apenas regiões JSON da aba UMA vez por quadro. Tabelas
        // grandes preservam a virtualização de linhas e o motor colunar.
        let tabelas_visuais = self.workbook.aba(self.aba)
            .map(|aba| crate::app::tabelas::visuais(&aba.sheet))
            .unwrap_or_default();
        let dinamicas_visuais = self.workbook.aba(self.aba)
            .map(|aba| crate::dinamica_visual::com_cache(&aba.sheet,aba.versao))
            .unwrap_or_default();
        let dinamica_acento = feather_ui::tema::COR_AZUL_ACENTO();
        // Apenas uma normalização por frame, mesmo com muitas colunas.
        let (bg, header, texto, fraco, borda, ativo, selecao) = if self.escuro {
            (Color::rgb8(34,37,42), Color::rgb8(45,49,55),
             Color::rgb8(234,237,242), Color::rgb8(167,178,190),
             Color::rgb8(58,64,72), Color::rgb8(79,179,173),
             Color::rgb8(79, 179, 173).alpha(0.18))
        } else {
            (Color::WHITE, Color::rgb8(244,246,248),
             Color::rgb8(30,35,40), Color::rgb8(85,96,110),
             Color::rgb8(221,226,232), Color::rgb8(11,87,208),
             Color::rgb8(11, 87, 208).alpha(0.09))
        };
        // Cores discretas, com contraste suficiente em ambos os temas.
        // A cruz é feita por célula visível, sem percorrer linhas fora do viewport.
        let (realce_eixo, realce_centro) = if self.escuro {
            (Color::rgb8(79, 179, 173).alpha(0.12),
             Color::rgb8(79, 179, 173).alpha(0.25))
        } else {
            (Color::rgb8(11, 87, 208).alpha(0.075),
             Color::rgb8(11, 87, 208).alpha(0.17))
        };
        // Divisórias dos cabeçalhos, independentes da opção de linhas de grade.
        // Mantém ~1 px na tela mesmo com zoom diferente de 100%.
        let espessura = 1.0 / self.zoom.max(0.01);
        let divisoria = if self.cabecalho_escuro {
            Color::rgb8(77, 84, 93)
        } else {
            Color::rgb8(218, 220, 224)
        };
        let divisoria_fixa = if self.escuro {
            Color::rgb8(111, 122, 136)
        } else {
            Color::rgb8(174, 184, 197)
        };
        f.fill_rectangle(Point::ORIGIN, bounds.size(), bg);
        // Cabeçalhos (letras e números) seguem o tema da interface; as células ficam claras.
        let (header, fraco_cab, ativo_cab, selecao_cab) = if self.cabecalho_escuro {
            (Color::rgb8(45,49,55), Color::rgb8(167,178,190), Color::rgb8(79,179,173), Color::rgb8(79,179,173).alpha(0.18))
        } else {
            (header, fraco, ativo, selecao)
        };
        f.fill_rectangle(Point::ORIGIN, Size::new(bounds.width,CABECALHO_COLUNA),header);
        f.fill_rectangle(Point::ORIGIN, Size::new(CABECALHO_LINHA,bounds.height),header);
        let coluna_atual = self.selecionada.1;
        let linha_atual = self.selecionada.0;
        // Matriz dinâmica: com a âncora ou uma vizinha selecionada, a área
        // derramada ganha um contorno fino na cor da seleção, como no Excel.
        let area_derramada = self.workbook.aba(self.aba)
            .and_then(|a| a.sheet.area_derramada_em(linha_atual, coluna_atual));
        let cor_derrame = ativo;
        // O Canvas continua pintando apenas as faixas visíveis. Uma faixa
        // congelada fica presa ao cabeçalho; a outra parte usa a origem rolável.
        let (colunas_visiveis, divisor_x) = self.colunas_visiveis(bounds.width);
        let (linhas_visiveis, divisor_y) = self.linhas_visiveis(bounds.height);
        // Tamanho inteiro da última linha/coluna cortada pela borda: o texto se
        // posiciona por ele e o recorte fica no pedaço visível (como no Excel).
        let (alturas_inteiras, larguras_inteiras) =
            self.extensoes_inteiras(&linhas_visiveis, &colunas_visiveis, bounds.size());
        // Parseia os metadados de mesclagem uma única vez por desenho.
        // O texto das células da região (inclusive âncora) será renderizado
        // SOMENTE na passagem final, já com o retângulo completo.
        let mesclas = self.workbook.aba(self.aba)
            .map(|aba| crate::app::mesclas::ler(&aba.sheet))
            .unwrap_or_default();
        // Caixas de seleção (Inserir > Checkbox): regras lidas uma vez por quadro.
        let caixas = self.workbook.aba(self.aba)
            .map(|aba| crate::caixas::Regras::da(&aba.sheet))
            .unwrap_or_default();
        // Depois do "ocultar até o fim" sobra uma área sem células: cinza, como no Excel,
        // para não parecer planilha vazia (#77).
        let folha_oculta = self.workbook.aba(self.aba).map(|a| &a.sheet);
        let fora = if self.escuro { Color::rgb8(24, 26, 30) } else { Color::rgb8(214, 219, 225) };
        if let Some(sheet) = folha_oculta {
            if let (Some(n), Some((i, &(c, x, _)))) = (sheet.colunas_ocultas_desde, colunas_visiveis.iter().enumerate().last()) {
                let fim = x + larguras_inteiras[i];
                if c + 1 == n && fim < bounds.width {
                    f.fill_rectangle(Point::new(fim, 0.0), Size::new(bounds.width - fim, bounds.height), fora);
                }
            }
            if let (Some(n), Some((i, &(r, y, _)))) = (sheet.linhas_ocultas_desde, linhas_visiveis.iter().enumerate().last()) {
                let fim = y + alturas_inteiras[i];
                if r + 1 == n && fim < bounds.height {
                    f.fill_rectangle(Point::new(0.0, fim), Size::new(bounds.width, bounds.height - fim), fora);
                }
            }
        }
        // Linha dupla no cabeçalho onde há linha/coluna oculta antes (#77).
        let marca_oculta = |f: &mut Frame, p: Point, t: Size, meio: Size| {
            f.fill_rectangle(p, t, fraco_cab);
            f.fill_rectangle(Point::new(p.x + (t.width - meio.width) / 2.0, p.y + (t.height - meio.height) / 2.0), meio, header);
        };
        let texto_mesclado = mesclas_visuais::mascarar(
            &mesclas, &linhas_visiveis, &colunas_visiveis,
        );
        // As bordas das células vão por último, centradas na linha de grade: a célula vizinha
        // (desenhada depois) e a própria grade não as cobrem, e duas bordas no mesmo lado
        // viram uma linha só (#69).
        let mut tracos_borda: Vec<(Point, Size, Color)> = Vec::new();
        for (ic, &(c, x, desenhar_w)) in colunas_visiveis.iter().enumerate() {
            let w = larguras_inteiras[ic];
            if c == coluna_atual {
                f.fill_rectangle(Point::new(x,0.0),Size::new(desenhar_w,CABECALHO_COLUNA),selecao_cab);
            }
            // Limite entre letras consecutivas (A/B, B/C...), no cabeçalho.
            // Ao rolar horizontalmente, usa x da coluna atualmente visível.
            if let Some(divisor_x) = divisoria_visivel(x, desenhar_w, bounds.width, espessura) {
                f.fill_rectangle(
                    Point::new(divisor_x, 0.0),
                    Size::new(espessura, CABECALHO_COLUNA.min(bounds.height)),
                    divisoria,
                );
            }
            if c > 0 && folha_oculta.is_some_and(|s| s.coluna_oculta(c - 1)) {
                marca_oculta(&mut f, Point::new(x - espessura, 0.0),
                    Size::new(3.0 * espessura, CABECALHO_COLUNA), Size::new(espessura, CABECALHO_COLUNA));
            }
            // Recortada na coluna: a letra da última, cortada, não invade a
            // barra de rolagem (texto do Canvas fica por cima das trilhas).
            recortar_celula(&mut f, self.zoom,
                Rectangle::new(Point::new(x, 0.0), Size::new(desenhar_w, CABECALHO_COLUNA)),
                |frame| frame.fill_text(Text {
                    content: col_name(c),
                    position: Point::new(w/2.0, CABECALHO_COLUNA/2.0),
                    color: if c == coluna_atual { ativo_cab } else { fraco_cab },
                    size: crate::compat::Pixels(12.0),
                    horizontal_alignment: pelicano::Align::Center,
                    vertical_alignment: pelicano::Align::Center,
                    ..Text::default()
                }));
            for (il, &(r, y, desenhar_h)) in linhas_visiveis.iter().enumerate() {
                let h = alturas_inteiras[il];
                let estilo = self.workbook.aba(self.aba)
                    .and_then(|a|a.sheet.estilo_resolvido(r,c));
                // Cor condicional tem prioridade VISUAL, sem alterar estilo
                // nem valor armazenado. Calcula somente as células do quadro.
                let cond = self.condicional.borrow_mut().formato(self.workbook,self.aba,r,c);
                // TBL-013: somente estilos de células VISÍVEIS passam pelo
                // contrato Tables; fórmulas e armazenamento permanecem no core.
                let cond_estilo = cond.as_ref().and_then(|f| f.style.as_ref());
                let projetado = crate::estilos_comuns::projetar_estilo(
                    estilo.as_ref(), cond_estilo,
                );
                let cor_fundo = projetado.comum.basico.fundo.map(|[r, g, b, a]|
                    Color::rgb8(r, g, b).alpha(a as f32 / 255.0));
                // Fundo de TabelaDaAba é APENAS visual; estilos explícitos e
                // formatação condicional permanecem com precedência.
                let fundo_tabela = crate::app::tabelas::fundo(&tabelas_visuais, r, c, self.escuro);
                let fundo_dinamica = crate::dinamica_visual::fundo(
                    &dinamicas_visuais, r, c, self.escuro, dinamica_acento);
                let grupo_recolhido = crate::dinamica_visual::recolhida(&dinamicas_visuais,r,c);
                if let Some(bg) = cor_fundo.or(fundo_tabela).or(fundo_dinamica) {
                    f.fill_rectangle(Point::new(x,y),Size::new(desenhar_w,desenhar_h),bg);
                }
                // Sobrepõe o fundo (inclusive formatação condicional), mas
                // abaixo de barras, texto, bordas e seleção para não os ocultar.
                let cor_cruz = match realce_cruz((r, c), self.selecionada, self.realce_eixos) {
                    RealceCruz::Nenhum => None,
                    RealceCruz::Eixo => Some(realce_eixo),
                    RealceCruz::Cruzamento => Some(realce_centro),
                };
                if let Some(cor) = cor_cruz {
                    f.fill_rectangle(Point::new(x, y), Size::new(desenhar_w, desenhar_h), cor);
                }
                if let Some(barra) = cond.as_ref().and_then(|v|v.bar.as_ref()) {
                    if let Some(cor) = cor_hex(&barra.color) {
                        let inicio = barra.from.clamp(0.0,1.0) as f32;
                        let fim = barra.to.clamp(0.0,1.0) as f32;
                        let largura_barra = (fim-inicio).max(0.0)*desenhar_w;
                        if largura_barra > 0.0 {
                            f.fill_rectangle(
                                Point::new(x+inicio*desenhar_w,y+desenhar_h*0.14),
                                Size::new(largura_barra,desenhar_h*0.72),cor,
                            );
                        }
                    }
                }
                if (r >= self.area.r0 && r <= self.area.r1 && c >= self.area.c0 && c <= self.area.c1)
                    || self.areas_extras.iter().any(|a| a.contains(r, c)) {
                    f.fill_rectangle(Point::new(x,y),Size::new(desenhar_w,desenhar_h),selecao);
                }
                if let Some(area) = area_derramada.as_ref().filter(|a| a.contains(r, c)) {
                    let espessura = 2.0;
                    if r == area.r0 {
                        f.fill_rectangle(Point::new(x,y),Size::new(desenhar_w,espessura),cor_derrame);
                    }
                    if r == area.r1 {
                        f.fill_rectangle(Point::new(x,y+desenhar_h-espessura),Size::new(desenhar_w,espessura),cor_derrame);
                    }
                    if c == area.c0 {
                        f.fill_rectangle(Point::new(x,y),Size::new(espessura,desenhar_h),cor_derrame);
                    }
                    if c == area.c1 {
                        f.fill_rectangle(Point::new(x+desenhar_w-espessura,y),Size::new(espessura,desenhar_h),cor_derrame);
                    }
                }
                // Contornos das referências presentes no rascunho, com cores alternadas.
                if let Some(indice)=self.refs_formula.iter().position(|caixa| {
                    let area = &caixa.rect;
                    r>=area.r0 && r<=area.r1 && c>=area.c0 && c<=area.c1
                }) {
                    let cores=[
                        Color::rgb8(43,104,214),Color::rgb8(18,157,107),
                        Color::rgb8(217,119,6),Color::rgb8(147,51,234),
                        Color::rgb8(220,38,38),Color::rgb8(8,145,178),
                    ];
                    let cor=cores[indice%cores.len()];
                    let area = &self.refs_formula[indice].rect;
                    let espessura = 2.0;
                    if r == area.r0 {
                        f.fill_rectangle(Point::new(x,y),Size::new(desenhar_w,espessura),cor);
                    }
                    if r == area.r1 {
                        f.fill_rectangle(Point::new(x,y+desenhar_h-espessura),Size::new(desenhar_w,espessura),cor);
                    }
                    if c == area.c0 {
                        f.fill_rectangle(Point::new(x,y),Size::new(espessura,desenhar_h),cor);
                    }
                    if c == area.c1 {
                        f.fill_rectangle(Point::new(x+desenhar_w-espessura,y),Size::new(espessura,desenhar_h),cor);
                    }
                    // Quadradinho nas quinas: é por elas que a área estica.
                    let q = (9.0/self.zoom.max(0.25)).min(desenhar_w/2.0).min(desenhar_h/2.0);
                    for (lin, col, embaixo, direita) in [(area.r0, area.c0, false, false), (area.r0, area.c1, false, true),
                        (area.r1, area.c0, true, false), (area.r1, area.c1, true, true)] {
                        if (r, c) == (lin, col) {
                            let qx = if direita { x+desenhar_w-q } else { x };
                            let qy = if embaixo { y+desenhar_h-q } else { y };
                            f.fill_rectangle(Point::new(qx,qy),Size::new(q,q),cor);
                        }
                    }
                }
                if (r,c) == self.selecionada {
                    f.fill_rectangle(Point::new(x,y),Size::new(desenhar_w,2.0),ativo);
                    f.fill_rectangle(Point::new(x,y+desenhar_h-2.0),Size::new(desenhar_w,2.0),ativo);
                    f.fill_rectangle(Point::new(x,y),Size::new(2.0,desenhar_h),ativo);
                    f.fill_rectangle(Point::new(x+desenhar_w-2.0,y),Size::new(2.0,desenhar_h),ativo);
                }
                let esconder = cond.as_ref().and_then(|f|f.bar.as_ref())
                    .is_some_and(|b|b.hide_value);
                // Caixa de seleção: no lugar do VERDADEIRO/FALSO, a caixa centrada.
                let caixa = (!self.mostrar_formulas).then(|| caixas.caixa_em(r, c)).flatten()
                    .and_then(|(m, d)| self.workbook.aba(self.aba)
                        .and_then(|a| crate::caixas::estado(&a.sheet, r, c, m, d)));
                if let Some(marcada) = caixa {
                    desenhar_caixa(&mut f, x, y, desenhar_w, desenhar_h, marcada, self.escuro, ativo);
                }
                let val = if caixa.is_some() {
                    String::new()
                } else if texto_mesclado.contains(&(r, c)) {
                    // Não desenhar texto de célula mesclada duas vezes.
                    // Valores persistem e voltam a ser vistos ao desmesclar.
                    String::new()
                } else if esconder && !self.mostrar_formulas {
                    String::new()
                } else if self.mostrar_formulas {
                    // Totais virtuais de tabela não substituem o texto da fórmula.
                    self.texto(r, c)
                } else {
                    crate::app::tabelas::total_texto(self.workbook, self.aba, &tabelas_visuais, r, c)
                        .unwrap_or_else(|| self.texto(r,c))
                };
                // Imagem dentro da célula (=IMAGEM, "Colocar na célula" do
                // .xlsx): a figura no lugar do texto, sem distorcer.
                let val = match (!self.mostrar_formulas).then(|| crate::imagem_celula::imagem(&val)).flatten() {
                    Some((handle, iw, ih)) => {
                        let (ix, iy, lw, lh) = crate::imagem_celula::encaixe(x, y, desenhar_w, desenhar_h, iw, ih);
                        f.draw_image(Rectangle::new(Point::new(ix, iy), Size::new(lw, lh)), &handle);
                        String::new()
                    }
                    None => val,
                };
                if !val.is_empty() {
                    // Links importados do XLSX e URLs digitadas no CSV têm
                    // aparência clicável, sem criar estilos por célula.
                    let link_visivel = estilo.as_ref()
                        .and_then(|s| s.link.as_deref())
                        .and_then(crate::app::link_ui::normalizar_link)
                        .is_some()
                        || crate::app::link_ui::normalizar_link(&val).is_some();
                    let ajuste = self.workbook.aba(self.aba)
                        .map(|a| ajustes_texto(&a.sheet,r,c)).unwrap_or_default();
                    let val = if ajuste.vertical {
                        val.chars().take(120).map(|ch| ch.to_string()).collect::<Vec<_>>().join("\n")
                    } else { val };
                    // Reticências somente para visualização; valor real permanece no motor.
                    let limite = if ajuste.vertical {
                        ((h - 8.0) / 11.0).floor().max(2.0) as usize
                    } else { ((w - 16.0)/7.1).floor().max(2.0) as usize };
                    let curto = if ajuste.quebrar && !ajuste.reduzir && !ajuste.vertical {
                        // shortcut: largura por caractere estimada (meia letra); medir o texto se a quebra sair torta.
                        let letra = estilo.as_ref().and_then(|s| s.font_size).unwrap_or(self.tamanho_fonte_padrao as f32).max(6.0);
                        let largura = (((w - 16.0) / (letra * 0.5)).floor().max(2.0)) as usize;
                        let linhas = (((h - 4.0) / (letra * 1.3)).floor().max(1.0)) as usize;
                        comentarios::quebrar(val.trim_end(), largura, linhas).join("\n")
                    } else if !ajuste.reduzir && !ajuste.vertical {
                        val.split('\n').map(|linha| {
                            if linha.chars().count() > limite {
                                format!("{}…", linha.chars().take(limite - 1).collect::<String>())
                            } else { linha.to_owned() }
                        }).collect::<Vec<_>>().join("\n")
                    } else { val };
                    // Formato personalizado: `*x` enche a largura e a seção pode ter cor.
                    let extra = self.formato_extra(r, c, estilo.as_ref()).filter(|e| e.texto == curto);
                    let curto = match extra.as_ref() {
                        Some(e) if !ajuste.reduzir && !ajuste.vertical => e.preenchido(limite),
                        _ => curto,
                    };
                    let alinhar = self.alinhar(r,c);
                    let tx = match alinhar {
                        pelicano::Align::End => x+w-8.0-ajuste.recuo*7.,
                        pelicano::Align::Center => x+w/2.0,
                        pelicano::Align::Start => x+8.0+ajuste.recuo*7.
                            + if grupo_recolhido.is_some() { 17.0 } else { 0.0 },
                    };
                    let cabecalho_tabela = crate::app::tabelas::cabecalho(&tabelas_visuais, r, c);
                    let cabecalho_dinamica = crate::dinamica_visual::cabecalho(&dinamicas_visuais,r,c);
                    let destaque_dinamica = crate::dinamica_visual::destaque(&dinamicas_visuais,r,c);
                    let cor_texto = projetado.comum.basico.texto
                        .map(|[r, g, b, a]| Color::rgb8(r, g, b).alpha(a as f32 / 255.0))
                        .unwrap_or_else(|| {
                            if cabecalho_tabela || cabecalho_dinamica {
                                cor_contraste(cor_fundo.or(fundo_tabela).or(fundo_dinamica), self.escuro)
                            } else {
                                texto
                            }
                        });
                    let cor_texto = if link_visivel && projetado.comum.basico.texto.is_none() {
                        ativo
                    } else { cor_texto };
                    // Cor da seção ([Vermelho]) vence a da fonte; a condicional vence as duas.
                    let cor_texto = match extra.as_ref().and_then(|e| e.cor) {
                        Some([vr, vg, vb]) if cond_estilo.and_then(|s| s.color.as_ref()).is_none() =>
                            Color::rgb8(vr, vg, vb),
                        _ => cor_texto,
                    };
                    let familia = projetado.comum.basico.fonte.as_deref().unwrap_or(self.fonte_padrao);
                    let fonte = pelicano::Font {
                        family: if familia.to_ascii_lowercase().contains("mono") {
                            Some("monospace")
                        } else if familia.contains("serif") || familia.contains("Georgia") ||
                            familia.contains("Times New Roman") {
                            Some("serif")
                        } else { None },
                        weight: if projetado.comum.basico.negrito || cabecalho_tabela || destaque_dinamica {
                            700
                        } else { 400 },
                        italic: projetado.comum.basico.italico,
                        ..pelicano::Font::DEFAULT
                    };
                    let alinhamento_vertical = match projetado.comum.layout.vertical {
                        feather_tables::AlinhamentoVertical::Superior =>
                            pelicano::Align::Start,
                        feather_tables::AlinhamentoVertical::Inferior =>
                            pelicano::Align::End,
                        feather_tables::AlinhamentoVertical::Centro =>
                            pelicano::Align::Center,
                    };
                    let mut y_texto = match alinhamento_vertical {
                        pelicano::Align::Start => y + 3.0,
                        pelicano::Align::End => y + h - 3.0,
                        _ => y + h/2.0,
                    };
                    let mut tamanho_fonte = projetado.comum.basico.tamanho.unwrap_or(self.tamanho_fonte_padrao as f32);
                    if ajuste.reduzir && !ajuste.vertical {
                        let estimado=curto.chars().count() as f32 * tamanho_fonte * 0.54;
                        let disponivel=(w-16.0-ajuste.recuo*7.).max(6.);
                        if estimado > disponivel {
                            tamanho_fonte=(disponivel/(curto.chars().count().max(1) as f32*0.54))
                                .clamp(6., tamanho_fonte);
                        }
                    }
                    if ajuste.sobrescrito || ajuste.subscrito {
                        tamanho_fonte *= 0.75;
                        y_texto += if ajuste.subscrito {h*0.18} else {-h*0.18};
                    }
                    let largura_texto = (curto.chars().count() as f32 * tamanho_fonte * 0.54)
                        .min((w - 14.0).max(0.0));
                    let inicio_texto = match alinhar {
                        pelicano::Align::End => (tx - largura_texto).max(x + 4.0),
                        pelicano::Align::Center => (tx - largura_texto/2.0).max(x + 4.0),
                        pelicano::Align::Start => tx,
                    };
                    let desenho_texto = Text {
                        content: curto,
                        position: Point::new(tx, y_texto),
                        color: cor_texto,
                        size: crate::compat::Pixels(tamanho_fonte),
                        font: fonte,
                        horizontal_alignment: alinhar,
                        vertical_alignment: alinhamento_vertical,
                        ..Text::default()
                    };
                    let rotacao = estilo.as_ref().and_then(|s| s.rotation)
                        .unwrap_or(0).clamp(-90, 90);
                    if rotacao == 0 {
                        // O conteúdo de A29 não pode escapar para A30. Dentro do
                        // recorte a origem passa a ser o canto da célula.
                        recortar_celula(&mut f, self.zoom,
                            Rectangle::new(Point::new(x, y), Size::new(desenhar_w, desenhar_h)),
                            |frame| frame.fill_text(Text {
                                position: Point::new(tx - x, y_texto - y),
                                ..desenho_texto
                            }));
                    } else {
                        recortar_celula(&mut f, self.zoom,
                            Rectangle::new(Point::new(x, y), Size::new(desenhar_w, desenhar_h)), |frame| {
                            frame.with_save(|frame| {
                                frame.translate(crate::compat::Vector::new(w / 2.0, h / 2.0));
                                frame.rotate(crate::compat::Radians((rotacao as f32).to_radians()));
                                frame.fill_text(Text {
                                    position: Point::ORIGIN,
                                    horizontal_alignment: pelicano::Align::Center,
                                    vertical_alignment: pelicano::Align::Center,
                                    ..desenho_texto
                                });
                            });
                        });
                    }
                    if rotacao == 0 && (projetado.comum.basico.sublinhado || link_visivel) {
                        // y_texto é o CENTRO do texto: o traço vai logo abaixo da
                        // linha de base (antes, a +2 px, cortava as letras ao meio).
                        let base = y_texto + tamanho_fonte * 0.42;
                        f.fill_rectangle(Point::new(inicio_texto, base.min(y+h-2.0)),
                            Size::new(largura_texto, 1.0), cor_texto);
                        // Sublinhado duplo: segundo traço 3 px abaixo do primeiro.
                        if ajuste.sublinhado_duplo {
                            f.fill_rectangle(Point::new(inicio_texto, (base + 3.0).min(y+h-1.0)),
                                Size::new(largura_texto, 1.0), cor_texto);
                        }
                    }
                    if rotacao == 0 && projetado.riscado {
                        f.fill_rectangle(Point::new(inicio_texto, (y_texto - tamanho_fonte*0.22).max(y+2.0)),
                            Size::new(largura_texto, 1.0), cor_texto);
                    }
                }
                // Expansão/recolhimento de grupos renderizados como vetor:
                // nenhum glifo Unicode ou fonte adicional no Canvas.
                if let Some(recolhido) = grupo_recolhido {
                    let px=x+4.0;
                    let py=y+(desenhar_h-14.0).max(0.0)/2.0;
                    let cor=if self.escuro { Color::rgb8(169,196,215) }
                        else { Color::rgb8(58,92,121) };
                    let fundo=if self.escuro { Color::rgb8(51,61,70) }
                        else { Color::rgb8(239,245,250) };
                    f.fill_rectangle(Point::new(px,py),Size::new(14.0,14.0),fundo);
                    f.stroke(&canvas::Path::rectangle(Point::new(px,py),Size::new(14.0,14.0)),
                        canvas::Stroke::default().with_color(cor).with_width(1.0));
                    f.fill_rectangle(Point::new(px+3.0,py+6.5),Size::new(8.0,1.0),cor);
                    if recolhido {
                        f.fill_rectangle(Point::new(px+6.5,py+3.0),Size::new(1.0,8.0),cor);
                    }
                }
                // Funil real no cabeçalho, sem depender de glifos Unicode.
                // Contorno = sem filtro; preenchido = filtro aplicado.
                // Filtro de relatório da dinâmica: o mesmo funil, na célula do item.
                if let Some((_, filtrado)) = crate::dinamica_visual::filtro_na_grade(&dinamicas_visuais,r,c) {
                    desenhar_funil(
                        &mut f,
                        Point::new(x+desenhar_w-12.0, y+desenhar_h/2.0),
                        filtrado,
                        cor_contraste(cor_fundo, self.escuro),
                    );
                }
                if let Some(filtrado) = crate::app::tabelas::icone_filtro(&tabelas_visuais,r,c) {
                    desenhar_funil(
                        &mut f,
                        Point::new(x+desenhar_w-12.0, y+desenhar_h/2.0),
                        filtrado,
                        cor_contraste(cor_fundo.or(fundo_tabela), self.escuro),
                    );
                }
                // Seta de cálculo na célula ATIVA da linha de totais (Excel).
                if (r,c) == self.selecionada
                    && crate::app::tabelas::e_celula_de_total(&tabelas_visuais,r,c)
                {
                    let lado = (desenhar_h-4.0).clamp(10.0,18.0);
                    let bx = x+desenhar_w-lado-2.0;
                    let by = y+(desenhar_h-lado)/2.0;
                    let (fundo_b,borda_b,seta) = if self.escuro {
                        (Color::rgb8(58,64,72),Color::rgb8(110,120,132),Color::rgb8(220,226,232))
                    } else {
                        (Color::rgb8(250,250,250),Color::rgb8(150,158,168),Color::rgb8(60,64,67))
                    };
                    f.fill_rectangle(Point::new(bx,by),Size::new(lado,lado),fundo_b);
                    f.stroke(&canvas::Path::rectangle(Point::new(bx,by),Size::new(lado,lado)),
                        canvas::Stroke::default().with_color(borda_b).with_width(1.0));
                    let tri = canvas::Path::new(|p| {
                        let (cx,cy,m) = (bx+lado/2.0,by+lado/2.0,lado*0.22);
                        p.move_to(Point::new(cx-m,cy-m*0.6));
                        p.line_to(Point::new(cx+m,cy-m*0.6));
                        p.line_to(Point::new(cx,cy+m*0.8));
                        p.close();
                    });
                    f.fill(&tri,seta);
                }
                if let Some((topo,base,esq,dir,cor)) =
                    crate::app::tabelas::borda(&tabelas_visuais,r,c,self.escuro)
                {
                    let espessura=1.0 / self.zoom.max(0.01);
                    if topo {f.fill_rectangle(Point::new(x,y),Size::new(desenhar_w,espessura),cor);}
                    if base {f.fill_rectangle(Point::new(x,y+desenhar_h-espessura),Size::new(desenhar_w,espessura),cor);}
                    if esq {f.fill_rectangle(Point::new(x,y),Size::new(espessura,desenhar_h),cor);}
                    if dir {f.fill_rectangle(Point::new(x+desenhar_w-espessura,y),Size::new(espessura,desenhar_h),cor);}
                }
                // APP-010: projeta apenas o estilo da célula visível nos
                // tipos comuns de Tables, sem criar outra grade ou editar o
                // armazenamento colunar/formulas do FeatherSheet.
                let fallback = if self.escuro {
                    [167, 178, 190, 255]
                } else {
                    [85, 96, 110, 255]
                };
                let bordas = crate::bordas_comuns::adaptar_bordas(
                    estilo.as_ref().and_then(|e| e.borders.as_ref()), fallback
                );
                // O estilo fino/médio/grosso/tracejado vem do mesmo
                // EstiloParcial que a janela Formatar Células modifica.
                // Na ausência dele preserva 1,5 px da engine compartilhada.
                let tipo_traco = if estilo.as_ref().is_some_and(|s| s.borders.is_some()) {
                    self.workbook.aba(self.aba)
                        .and_then(|aba| aba.sheet.estilo_mesclado(r, c))
                        .and_then(|m| match m.get("borderLineStyle") {
                            Some(feathersheet_core::celulas::estilo_parcial::Campo::Texto(v)) =>
                                Some(v.clone()),
                            _ => None,
                        })
                } else {None};
                for (lado, segmento) in crate::bordas_comuns::segmentos(
                    bordas, x, y, desenhar_w, desenhar_h
                ).into_iter().enumerate() {
                    let Some(segmento) = segmento else { continue };
                    let [vr, vg, vb, va] = segmento.cor;
                    let cor = Color::rgb8(vr, vg, vb).alpha(va as f32 / 255.0);
                    let esp = match tipo_traco.as_deref() {
                        Some("Média") => 2.5,
                        Some("Grossa") => 4.0,
                        _ => segmento.largura.min(segmento.altura),
                    }.min(desenhar_w).min(desenhar_h).max(0.1);
                    // A divisa entre duas células é o pixel da linha de grade (x + w - 1).
                    let meio = esp / 2.0;
                    let (sx, sy, largura, altura) = match lado {
                        0 => (x - 0.5 - meio, y - 0.5 - meio, desenhar_w + esp, esp),
                        1 => (x + desenhar_w - 0.5 - meio, y - 0.5 - meio, esp, desenhar_h + esp),
                        2 => (x - 0.5 - meio, y + desenhar_h - 0.5 - meio, desenhar_w + esp, esp),
                        _ => (x - 0.5 - meio, y - 0.5 - meio, esp, desenhar_h + esp),
                    };
                    // Sem invadir os cabeçalhos de linha e de coluna.
                    let (sx, largura) = (sx.max(CABECALHO_LINHA), largura - (CABECALHO_LINHA - sx).max(0.0));
                    let (sy, altura) = (sy.max(CABECALHO_COLUNA), altura - (CABECALHO_COLUNA - sy).max(0.0));
                    if largura <= 0.0 || altura <= 0.0 { continue; }
                    if tipo_traco.as_deref() == Some("Tracejada") {
                        let horizontal = lado == 0 || lado == 2;
                        let comprimento = if horizontal {largura} else {altura};
                        let mut deslocamento = 0.0_f32;
                        while deslocamento < comprimento {
                            let parte = (comprimento - deslocamento).min(6.0);
                            if horizontal {
                                tracos_borda.push((Point::new(sx + deslocamento, sy), Size::new(parte, altura), cor));
                            } else {
                                tracos_borda.push((Point::new(sx, sy + deslocamento), Size::new(largura, parte), cor));
                            }
                            deslocamento += 10.0;
                        }
                    } else {
                        tracos_borda.push((Point::new(sx, sy), Size::new(largura, altura), cor));
                    }
                }
                if self.linhas_grade {
                    f.fill_rectangle(Point::new(x,y+h-1.0),Size::new(desenhar_w,1.0),borda);
                }
            }
            if self.linhas_grade {
                f.fill_rectangle(Point::new(x+desenhar_w-1.0,CABECALHO_COLUNA),
                    Size::new(1.0,bounds.height-CABECALHO_COLUNA),borda);
            }
        }
        for (p, t, cor) in tracos_borda {
            f.fill_rectangle(p, t, cor);
        }
        // Numeração das linhas é desenhada uma única vez (não por coluna).
        for (il, &(r, y, desenhar_h)) in linhas_visiveis.iter().enumerate() {
            let h = alturas_inteiras[il];
            if r == linha_atual {
                f.fill_rectangle(Point::new(0.0, y), Size::new(CABECALHO_LINHA, desenhar_h), selecao_cab);
            }
            // Limite entre números de linhas consecutivas (1/2, 2/3...).
            if let Some(divisor_y) = divisoria_visivel(y, desenhar_h, bounds.height, espessura) {
                f.fill_rectangle(
                    Point::new(0.0, divisor_y),
                    Size::new(CABECALHO_LINHA.min(bounds.width), espessura),
                    divisoria,
                );
            }
            if r > 0 && folha_oculta.is_some_and(|s| s.linha_oculta(r - 1)) {
                marca_oculta(&mut f, Point::new(0.0, y - espessura),
                    Size::new(CABECALHO_LINHA, 3.0 * espessura), Size::new(CABECALHO_LINHA, espessura));
            }
            // Centrado na altura INTEIRA e recortado no pedaço visível: a última
            // linha, cortada, mostra o número no tamanho normal (só cortado),
            // sem cair sobre o número da linha de cima nem sobre a barra.
            recortar_celula(&mut f, self.zoom,
                Rectangle::new(Point::new(0.0, y), Size::new(CABECALHO_LINHA, desenhar_h)),
                |frame| frame.fill_text(Text {
                    content: (r+1).to_string(),
                    position: Point::new(CABECALHO_LINHA-9.0, h/2.0),
                    color: if r == linha_atual { ativo_cab } else { fraco_cab },
                    size: crate::compat::Pixels(12.0),
                    horizontal_alignment: pelicano::Align::End,
                    vertical_alignment: pelicano::Align::Center,
                    ..Text::default()
                }));
        }
        // Separadores reais dos painéis fixos, sem cobrir textos ou células.
        if self.congelar_colunas > 0 && divisor_x > CABECALHO_LINHA {
            f.fill_rectangle(Point::new(divisor_x-1.5,CABECALHO_COLUNA),
                Size::new(2.5,(bounds.height-CABECALHO_COLUNA).max(0.0)),divisoria_fixa);
        }
        if self.congelar_linhas > 0 && divisor_y > CABECALHO_COLUNA {
            f.fill_rectangle(Point::new(CABECALHO_LINHA,divisor_y-1.5),
                Size::new((bounds.width-CABECALHO_LINHA).max(0.0),2.5),divisoria_fixa);
        }
        // Mesclagem visual do TS: elimina divisórias e textos internos no
        // retângulo visível, depois desenha SOMENTE o texto da célula âncora.
        // Calcula interseção com o viewport antes de somar alturas/larguras.
        if let Some(aba) = self.workbook.aba(self.aba) {
            for m in &mesclas {
                // Interseção APENAS com a grade desenhada: somar o intervalo
                // entre a linha congelada e A100000000 seria O(100 milhões).
                let cols: Vec<_> = colunas_visiveis.iter()
                    .filter(|(i,_,_)| *i >= m.c0 && *i <= m.c1).collect();
                let rows: Vec<_> = linhas_visiveis.iter()
                    .filter(|(i,_,_)| *i >= m.r0 && *i <= m.r1).collect();
                let (Some(primeira_col), Some(primeira_linha)) = (cols.first(), rows.first())
                    else { continue };
                let x = primeira_col.1;
                let y = primeira_linha.1;
                let w = cols.iter().map(|(_,_,w)| *w).sum::<f32>()
                    .min(bounds.width-x).max(0.0);
                let h = rows.iter().map(|(_,_,h)| *h).sum::<f32>()
                    .min(bounds.height-y).max(0.0);
                if w <= 0.0 || h <= 0.0 { continue; }
                // Extensão inteira (última linha/coluna cortada pela borda conta
                // inteira): o texto se posiciona por ela e o recorte usa `w`×`h`.
                let wt = colunas_visiveis.iter().zip(&larguras_inteiras)
                    .filter(|((i,_,_),_)| *i >= m.c0 && *i <= m.c1).map(|(_,l)| *l).sum::<f32>().max(w);
                let ht = linhas_visiveis.iter().zip(&alturas_inteiras)
                    .filter(|((i,_,_),_)| *i >= m.r0 && *i <= m.r1).map(|(_,a)| *a).sum::<f32>().max(h);
                // #36: rolada para a direita (ou para baixo), a mesclada começa fora da tela; o
                // texto se posiciona pela região inteira, como no Excel, e só o recorte é o visível.
                let (xo, yo, wt, ht) = {
                    let (xa, ya) = (modelo::inicio_x_da(&aba.sheet, primeira_col.0), modelo::inicio_y_da(&aba.sheet, primeira_linha.0));
                    let (x0, y0) = (modelo::inicio_x_da(&aba.sheet, m.c0), modelo::inicio_y_da(&aba.sheet, m.r0));
                    let wt_toda = modelo::inicio_x_da(&aba.sheet, m.c1 + 1) - x0;
                    let ht_toda = modelo::inicio_y_da(&aba.sheet, m.r1 + 1) - y0;
                    (x - (xa - x0).max(0.0), y - (ya - y0).max(0.0), wt_toda.max(wt), ht_toda.max(ht))
                };
                // #97: cruzando o painel congelado, o pedaço fixo e o rolado não são uma faixa
                // contínua da planilha; centralizar pela região inteira tirava o texto da tela.
                let cruza_linhas = mesclas_visuais::cruza_congelado(self.congelar_linhas, m.r0, m.r1);
                let cruza_colunas = mesclas_visuais::cruza_congelado(self.congelar_colunas, m.c0, m.c1);
                let (yo, ht) = if cruza_linhas { (y, h) } else { (yo, ht) };
                let (xo, wt) = if cruza_colunas { (x, w) } else { (xo, wt) };
                let estilo = aba.sheet.estilo_resolvido(m.r0, m.c0);
                let visual = crate::estilos_comuns::projetar_estilo(estilo.as_ref(), None);
                let cor_fundo = visual.comum.basico.fundo
                    .map(|[r,g,b,a]| Color::rgb8(r, g, b).alpha(a as f32 / 255.0))
                    .or_else(|| crate::app::tabelas::fundo(&tabelas_visuais, m.r0, m.c0, self.escuro))
                    .unwrap_or(bg);
                f.fill_rectangle(Point::new(x,y), Size::new(w,h), cor_fundo);
                if m.r0 <= self.area.r1 && m.r1 >= self.area.r0
                    && m.c0 <= self.area.c1 && m.c1 >= self.area.c0 {
                    f.fill_rectangle(Point::new(x,y), Size::new(w,h), selecao);
                }
                let valor = self.texto(m.r0, m.c0);
                if !valor.is_empty() {
                    let link_visivel = estilo.as_ref().and_then(|s| s.link.as_deref())
                        .and_then(crate::app::link_ui::normalizar_link).is_some()
                        || crate::app::link_ui::normalizar_link(&valor).is_some();
                    let ajuste=ajustes_texto(&aba.sheet,m.r0,m.c0);
                    let valor=if ajuste.vertical {
                        valor.chars().take(120).map(|ch|ch.to_string()).collect::<Vec<_>>().join("\n")
                    } else {valor};
                    let limite = if ajuste.vertical {
                        ((ht-8.)/11.).floor().max(2.) as usize
                    } else {((wt-16.)/7.1).floor().max(2.) as usize};
                    let curto = if ajuste.quebrar && !ajuste.reduzir && !ajuste.vertical {
                        // shortcut: largura por caractere estimada (meia letra); medir o texto se a quebra sair torta.
                        let letra = estilo.as_ref().and_then(|s| s.font_size).unwrap_or(self.tamanho_fonte_padrao as f32).max(6.0);
                        let largura = (((wt - 16.0) / (letra * 0.5)).floor().max(2.0)) as usize;
                        let linhas = (((ht - 4.0) / (letra * 1.3)).floor().max(1.0)) as usize;
                        comentarios::quebrar(valor.trim_end(), largura, linhas).join("\n")
                    } else if !ajuste.reduzir && !ajuste.vertical {
                        valor.split('\n').map(|linha| {
                            if linha.chars().count() > limite {
                                format!("{}…", linha.chars().take(limite - 1).collect::<String>())
                            } else { linha.to_owned() }
                        }).collect::<Vec<_>>().join("\n")
                    } else { valor };
                    let extra = self.formato_extra(m.r0, m.c0, estilo.as_ref()).filter(|e| e.texto == curto);
                    let curto = match extra.as_ref() {
                        Some(e) if !ajuste.reduzir && !ajuste.vertical => e.preenchido(limite),
                        _ => curto,
                    };
                    let alinhar = self.alinhar(m.r0, m.c0);
                    let tx = match alinhar {
                        pelicano::Align::End => xo+wt-8.0-ajuste.recuo*7.,
                        pelicano::Align::Center => xo+wt/2.0,
                        _ => xo+8.0+ajuste.recuo*7.,
                    };
                    let cor_texto = visual.comum.basico.texto
                        .map(|[r,g,b,a]| Color::rgb8(r, g, b).alpha(a as f32 / 255.0))
                        .unwrap_or(texto);
                    let cor_texto = if link_visivel && visual.comum.basico.texto.is_none() {
                        ativo
                    } else { cor_texto };
                    let cor_texto = extra.as_ref().and_then(|e| e.cor)
                        .map_or(cor_texto, |[vr, vg, vb]| Color::rgb8(vr, vg, vb));
                    let mut tamanho_fonte=visual.comum.basico.tamanho.unwrap_or(13.0);
                    if ajuste.reduzir && !ajuste.vertical {
                        let disponivel=(wt-16.0-ajuste.recuo*7.).max(6.);
                        let estimado=curto.chars().count() as f32*tamanho_fonte*0.54;
                        if estimado > disponivel {
                            tamanho_fonte=(disponivel/(curto.chars().count().max(1) as f32*0.54))
                                .clamp(6.,tamanho_fonte);
                        }
                    }
                    let mut y_texto=yo+ht/2.0;
                    if ajuste.sobrescrito || ajuste.subscrito {
                        tamanho_fonte*=0.75;
                        y_texto+=if ajuste.subscrito {ht*0.18} else {-ht*0.18};
                    }
                    let desenho_texto = Text {
                        content: curto.clone(),
                        position: Point::new(tx, y_texto),
                        color: cor_texto,
                        size: crate::compat::Pixels(tamanho_fonte),
                        font: pelicano::Font {
                            weight: if visual.comum.basico.negrito {
                                700
                            } else { 400 },
                            ..pelicano::Font::DEFAULT
                        },
                        horizontal_alignment: alinhar,
                        vertical_alignment: pelicano::Align::Center,
                        ..Text::default()
                    };
                    let rotacao = estilo.as_ref().and_then(|s| s.rotation)
                        .unwrap_or(0).clamp(-90, 90);
                    if rotacao == 0 {
                        // Origem relativa à região recortada, como acima.
                        recortar_celula(&mut f, self.zoom, Rectangle::new(Point::new(x, y), Size::new(w, h)),
                            |frame| frame.fill_text(Text {
                                position: Point::new(tx - x, y_texto - y),
                                ..desenho_texto
                            }));
                        if link_visivel || visual.comum.basico.sublinhado {
                            let largura_texto=(curto.chars().count() as f32*tamanho_fonte*0.54)
                                .min((w-14.0).max(0.0));
                            let inicio=match alinhar {
                                pelicano::Align::End => (tx-largura_texto).max(x+4.0),
                                pelicano::Align::Center => (tx-largura_texto/2.0).max(x+4.0),
                                _ => tx.max(x+4.0),
                            };
                            let base=y_texto+tamanho_fonte*0.42;
                            f.fill_rectangle(Point::new(inicio,base.min(y+h-2.0)),
                                Size::new(largura_texto,1.0),cor_texto);
                            if ajuste.sublinhado_duplo {
                                f.fill_rectangle(Point::new(inicio,(base+3.0).min(y+h-1.0)),
                                    Size::new(largura_texto,1.0),cor_texto);
                            }
                        }
                    } else {
                        recortar_celula(&mut f, self.zoom, Rectangle::new(Point::new(x, y), Size::new(w, h)), |frame| {
                            frame.with_save(|frame| {
                                frame.translate(crate::compat::Vector::new(wt / 2.0, ht / 2.0));
                                frame.rotate(crate::compat::Radians((rotacao as f32).to_radians()));
                                frame.fill_text(Text {
                                    position: Point::ORIGIN,
                                    horizontal_alignment: pelicano::Align::Center,
                                    vertical_alignment: pelicano::Align::Center,
                                    ..desenho_texto
                                });
                            });
                        });
                    }
                }
                let cor = if m.contains(self.selecionada.0, self.selecionada.1) {
                    ativo
                } else { borda };
                let esp = if m.contains(self.selecionada.0, self.selecionada.1) { 2.0 } else { 1.0 };
                f.fill_rectangle(Point::new(x,y), Size::new(w,esp), cor);
                f.fill_rectangle(Point::new(x,y+h-esp), Size::new(w,esp), cor);
                f.fill_rectangle(Point::new(x,y), Size::new(esp,h), cor);
                f.fill_rectangle(Point::new(x+w-esp,y), Size::new(esp,h), cor);
            }
        }

        // Duas divisórias fixas mais fortes, como nas planilhas Google:
        // 1) entre a numeração das linhas e as células;
        // 2) entre as letras das colunas e as células.
        // Devem sobreviver ao destaque de cabeçalhos e ao modo sem grade.
        let borda_fixa = 1.5 * espessura;
        if bounds.width >= CABECALHO_LINHA {
            f.fill_rectangle(
                Point::new(CABECALHO_LINHA - borda_fixa, 0.0),
                Size::new(borda_fixa, bounds.height),
                divisoria_fixa,
            );
        }
        if bounds.height >= CABECALHO_COLUNA {
            f.fill_rectangle(
                Point::new(0.0, CABECALHO_COLUNA - borda_fixa),
                Size::new(bounds.width, borda_fixa),
                divisoria_fixa,
            );
        }
        self.desenhar_topicos(&mut f, &colunas_visiveis, &linhas_visiveis, fraco, header);
        self.desenhar_setas(&mut f, &colunas_visiveis, &linhas_visiveis, bounds.size(), ativo);

        // Setinha de seleção e lista flutuante DENTRO do Canvas, ancoradas
        // à célula selecionada. Não aumentam a altura da planilha nem da Ribbon.
        if self.opcoes_lista().is_some() {
            if let Some((x, y, w, h)) = self.retangulo_celula_ativa(bounds.size()) {
                let acento = if self.escuro { Color::rgb8(122, 180, 248) }
                    else { Color::rgb8(35, 112, 204) };
                let fundo_botao = if self.escuro { Color::rgb8(56, 65, 79) }
                    else { Color::rgb8(229, 238, 252) };
                f.fill_rectangle(Point::new(x + w - 21.0, y + 3.0),
                    Size::new(18.0, (h - 6.0).min(22.0)), fundo_botao);
                let seta = canvas::Path::new(|p| {
                    let cx = x + w - 12.0;
                    let cy = y + h.min(28.0) / 2.0;
                    p.move_to(Point::new(cx - 4.0, cy - 2.0));
                    p.line_to(Point::new(cx + 4.0, cy - 2.0));
                    p.line_to(Point::new(cx, cy + 2.5));
                    p.close();
                });
                f.fill(&seta, acento);
            }
        }
        // Prévia visual apenas no viewport. Não altera a fórmula nem o Workbook.
        if let Some(arraste) = &state.referencia {
            if let Some(caixa) = self.refs_formula.iter().find(|c| c.start == arraste.start) {
                let area = drag_rect(&caixa.rect, arraste.pegada, arraste.inicio, arraste.destino);
                let cor = Color::rgb8(9, 139, 113);
                let e = 2.0 / self.zoom.max(0.25);
                for &(c, x, w) in &colunas_visiveis {
                    if c < area.c0 || c > area.c1 { continue; }
                    for &(r, y, h) in &linhas_visiveis {
                        if r < area.r0 || r > area.r1 { continue; }
                        if r == area.r0 { f.fill_rectangle(Point::new(x,y),Size::new(w,e),cor); }
                        if r == area.r1 { f.fill_rectangle(Point::new(x,y+h-e),Size::new(w,e),cor); }
                        if c == area.c0 { f.fill_rectangle(Point::new(x,y),Size::new(e,h),cor); }
                        if c == area.c1 { f.fill_rectangle(Point::new(x+w-e,y),Size::new(e,h),cor); }
                    }
                }
            }
        }
        // Os balões ficam na CamadaLista (canvas de cima): aqui, só os
        // triângulos e os círculos.
        self.desenhar_comentarios(&mut f, &colunas_visiveis, &linhas_visiveis, bounds.size(), false);
        self.desenhar_validacao(&mut f, &colunas_visiveis, &linhas_visiveis, bounds.size(), false);
        self.desenhar_impressao(&mut f, bounds, &colunas_visiveis, &linhas_visiveis);
        self.desenhar_alca(&mut f, bounds.size(), state.alca.as_ref(), &colunas_visiveis, &linhas_visiveis, ativo, bg);
        self.desenhar_copiada(&mut f, &colunas_visiveis, &linhas_visiveis, ativo);
        if let Some(a) = state.dimensao { desenhar_guia_dimensao(&mut f, bounds, a, ativo, self.zoom); }
        f.fim();
        }
        self.desenhar_barras(renderer, tela, state.barra);
        crate::vigia::conferir(|| "desenhar a grade (calcula as células visíveis)".into(), inicio_vigia.elapsed());
    }

    fn update(
        &self, state: &mut Self::State, event: &Event, bounds: Rectangle,
        cursor: Option<Point>, publish: &mut dyn FnMut(Msg),
    ) -> Captured {
        let (capturado, msg) = self.atualizar(state, event, bounds, cursor);
        if let Some(msg) = msg { publish(msg); }
        capturado
    }
    fn mouse_interaction(
        &self, state:&Self::State, bounds:Rectangle, cursor:Option<Point>,
    ) -> mouse::Interaction {
        let icone = self.icone_do_mouse(state, bounds, cursor);
        self.cache.cursor.set(icone);
        icone
    }
}

impl Grade<'_> {
    fn icone_do_mouse(
        &self, _state:&EstadoMouse, bounds:Rectangle, cursor:Option<Point>,
    ) -> mouse::Interaction {
        if let Some(p) = local(cursor, bounds) {
            if p.x >= bounds.width - barras::ESPESSURA || p.y >= bounds.height - barras::ESPESSURA {
                return mouse::Interaction::default();
            }
        }
        if let Some(a) = _state.dimensao {
            return if a.coluna { mouse::Interaction::EwResize } else { mouse::Interaction::NsResize };
        }
        // Borda e quinas de uma referência (ou do contorno do gráfico): agarrar.
        let referencia = _state.referencia.or_else(|| local(cursor, bounds)
            .and_then(|p| self.pegar_referencia(p, bounds.size())));
        if let Some(r) = referencia {
            return match r.pegada {
                feathersheet_core::formula::ref_drag::Pegada::Mover if _state.referencia.is_some() => mouse::Interaction::Grabbing,
                feathersheet_core::formula::ref_drag::Pegada::Mover => mouse::Interaction::Grab,
                feathersheet_core::formula::ref_drag::Pegada::Redimensionar => {
                    let area = self.refs_formula.iter().find(|c| c.start == r.start).map(|c| &c.rect);
                    // Quina de cima à esquerda ou de baixo à direita: diagonal ↖↘.
                    let diagonal = area.is_none_or(|a| (r.inicio.0 == a.r0) == (r.inicio.1 == a.c0));
                    if diagonal { mouse::Interaction::NwseResize } else { mouse::Interaction::NeswResize }
                }
            };
        }
        if let Some(p) = local(cursor, bounds) {
            let p = Point::new(p.x / self.zoom, p.y / self.zoom);
            let perto = |inicio: f32, tamanho: f32, ponto: f32| (ponto - (inicio + tamanho)).abs() <= FOLGA_DIVISORIA;
            if p.y < CABECALHO_COLUNA && p.x >= CABECALHO_LINHA
                && self.colunas_visiveis(bounds.width / self.zoom).0.iter().any(|&(_, x, w)| perto(x, w, p.x)) {
                return mouse::Interaction::EwResize;
            }
            if p.x < CABECALHO_LINHA && p.y >= CABECALHO_COLUNA
                && self.linhas_visiveis(bounds.height / self.zoom).0.iter().any(|&(_, y, h)| perto(y, h, p.y)) {
                return mouse::Interaction::NsResize;
            }
            return mouse::Interaction::Crosshair;
        }
        mouse::Interaction::default()
    }
}

impl Grade<'_> {
    /// O `update` do Iced: devolve se capturou e a mensagem (o `update` acima publica).
    fn atualizar(
        &self, state: &mut EstadoMouse, event: &Event,
        bounds: Rectangle, cursor: Option<Point>,
    ) -> (Captured, Option<Msg>) {
        match *event {
            Event::MousePressed(mouse::Button::Right) => {
                if let Some(point) = local(cursor, bounds) {
                    let p = Point::new(point.x / self.zoom, point.y / self.zoom);
                    // O menu é um overlay da área visível: clampa posição e
                    // tamanho ANTES de enviar o evento à App. As coordenadas
                    // do clique pertencem ao viewport Iced (sem zoom), e as
                    // da célula acima já foram convertidas com self.zoom.
                    let Some(popup) = crate::geometria_overlay::PosicaoPopup::contexto(
                        point.x, point.y, bounds.width, bounds.height,
                    ) else {
                        return (Captured::No, None);
                    };

                    if p.y < CABECALHO_COLUNA && p.x >= CABECALHO_LINHA {
                        for &(col, cx, w) in &self.colunas_visiveis(bounds.width/self.zoom).0 {
                            if p.x >= cx && p.x < cx+w {
                                return (Captured::Yes,
                                    Some(Msg::AbrirContextoNaGrade(Contexto::Coluna(col), popup)));
                            }
                        }
                    }
                    if p.x < CABECALHO_LINHA && p.y >= CABECALHO_COLUNA {
                        for &(lin, cy, h) in &self.linhas_visiveis(bounds.height/self.zoom).0 {
                            if p.y >= cy && p.y < cy+h {
                                return (Captured::Yes,
                                    Some(Msg::AbrirContextoNaGrade(Contexto::Linha(lin), popup)));
                            }
                        }
                    }
                    if let Some((r, c)) = self.celula_no_ponto(point, bounds.size()) {
                        return (Captured::Yes,
                            Some(Msg::AbrirContextoNaGrade(Contexto::Celula(r, c), popup)));
                    }
                }
            }
            Event::MousePressed(mouse::Button::Left)
                if local(cursor, bounds).is_some_and(|p| {
                    p.x >= bounds.width - barras::ESPESSURA || p.y >= bounds.height - barras::ESPESSURA
                }) => {
                let p = local(cursor, bounds).unwrap_or(Point::ORIGIN);
                if let Some((arrasto, msg)) = self.pressionar_barra(p, bounds.size()) {
                    state.barra = arrasto;
                    return (Captured::Yes, msg);
                }
            }
            Event::CursorMoved(position) if state.barra.is_some() => {
                let local = Point::new(position.x - bounds.x, position.y - bounds.y);
                let msg = state.barra.map(|a| self.arrastar_barra(a, local));
                return (Captured::Yes, msg);
            }
            Event::MouseReleased(mouse::Button::Left) if state.barra.is_some() => {
                state.barra = None;
                return (Captured::Yes, None);
            }
            Event::CursorMoved(position) if state.dimensao.is_some() => {
                if let Some(a) = &mut state.dimensao {
                    let local = if a.coluna { position.x - bounds.x } else { position.y - bounds.y };
                    a.atual = (local / self.zoom).max(a.inicio);
                }
                return (Captured::Yes, None);
            }
            Event::MouseReleased(mouse::Button::Left) if state.dimensao.is_some() => {
                let a = state.dimensao.take().expect("conferido no guarda");
                // Clique sem arrastar não muda nada (o duplo clique cuida do autoajuste).
                if a.tamanho() == a.original { return (Captured::Yes, None); }
                return (Captured::Yes, Some(Msg::DimensaoArrastada(a.coluna, a.indice, a.tamanho())));
            }
            Event::MousePressed(mouse::Button::Left)
                if local(cursor, bounds).is_some_and(|p| self.sobre_alca(p, bounds.size())) => {
                // Alça de preenchimento: duplo clique continua até o fim da
                // coluna vizinha; senão começa o arrasto com a prévia.
                let agora = std::time::Instant::now();
                if state.ultimo_clique_alca.take()
                    .is_some_and(|antes| agora.duration_since(antes) <= INTERVALO_DUPLO_CLIQUE) {
                    return (Captured::Yes,
                        Some(Msg::Preencher(crate::app::DirecaoPreenchimento::AteOFim)));
                }
                state.ultimo_clique_alca = Some(agora);
                state.alca = Some(alca::ArrastoAlca { destino: self.area.normalized() });
                return (Captured::Yes, None);
            }
            Event::CursorMoved(_) if state.alca.is_some() => {
                if let Some(destino) = local(cursor, bounds)
                    .and_then(|p| self.destino_da_alca(p, bounds.size())) {
                    if let Some(a) = &mut state.alca { a.destino = destino; }
                }
                return (Captured::Yes, None);
            }
            Event::MouseReleased(mouse::Button::Left) if state.alca.is_some() => {
                let a = state.alca.take().expect("conferido no guarda");
                if a.destino == self.area.normalized() { return (Captured::Yes, None); }
                // Arrastou: não é a primeira metade de um duplo clique.
                state.ultimo_clique_alca = None;
                return (Captured::Yes, Some(Msg::PreencherAlca(a.destino)));
            }
            Event::MousePressed(mouse::Button::Left) => {
                if let Some(point) = local(cursor, bounds) {
                    // A borda colorida de uma referência é arrastável; o interior
                    // continua selecionando células normalmente.
                    if let Some(referencia) = self.pegar_referencia(point, bounds.size()) {
                        state.referencia = Some(referencia);
                        return (Captured::Yes, Some(Msg::PegarReferencia));
                    }
                    let p = Point::new(point.x / self.zoom, point.y / self.zoom);
                    // Clique na caixa de seleção de qualquer célula: marca/desmarca.
                    if let Some((r, c)) = self.caixa_no_ponto(p, Size::new(bounds.width / self.zoom, bounds.height / self.zoom)) {
                        state.celulas = false;
                        return (Captured::Yes, Some(Msg::CaixaAlternar(r, c)));
                    }
                    // Prioridade ao dropdown antes de selecionar a célula
                    // que aparece geometricamente por baixo do menu.
                    if let Some(opcoes) = self.opcoes_lista() {
                        if let Some((x, y, w, h)) =
                            self.retangulo_celula_ativa(Size::new(
                                bounds.width / self.zoom, bounds.height / self.zoom))
                        {
                            if self.lista_aberta {
                                let viewport = Size::new(bounds.width / self.zoom, bounds.height / self.zoom);
                                if let Some(item) = self.item_da_lista(p, viewport, opcoes.len()) {
                                    return (Captured::Yes, item.and_then(|i| opcoes.get(i))
                                        .map(|v| Msg::ValidacaoListaEscolher(v.clone())));
                                }
                            }
                            if p.x >= x + w - 23.0 && p.x <= x + w &&
                                p.y >= y && p.y <= y + h {
                                return (Captured::Yes,
                                    Some(Msg::ValidacaoListaAlternar));
                            }
                        }
                    }
                    // [+]/[−] e botões de nível da estrutura de tópicos.
                    if p.x < CABECALHO_LINHA || p.y < CABECALHO_COLUNA {
                        let colunas = self.colunas_visiveis(bounds.width/self.zoom).0;
                        let linhas = self.linhas_visiveis(bounds.height/self.zoom).0;
                        if let Some(msg) = self.clique_topicos(p, &colunas, &linhas) {
                            return (Captured::Yes, Some(msg));
                        }
                    }
                    // Canto superior esquerdo: planilha inteira.
                    if p.y < CABECALHO_COLUNA && p.x < CABECALHO_LINHA {
                        return (Captured::Yes, Some(Msg::SelecionarTudo));
                    }
                    if p.y < CABECALHO_COLUNA && p.x >= CABECALHO_LINHA {
                        let colunas = self.colunas_visiveis(bounds.width/self.zoom).0;
                        // Divisória: duplo clique ajusta ao conteúdo (lote se selecionada).
                        if let Some(&(col, x, w)) = colunas.iter().find(|&&(_, x, w)|
                            (p.x - (x + w)).abs() <= FOLGA_DIVISORIA) {
                            return (Captured::Yes, clique_divisoria(state, true, col, x, x + w));
                        }
                        for &(col, x, w) in &colunas {
                            if p.x >= x && p.x < x+w {
                                if state.shift {
                                    return (Captured::Yes, Some(Msg::EstenderColuna(col)));
                                }
                                state.cabecalho = Some((true, col));
                                return (Captured::Yes, Some(Msg::SelecionarColuna(col)));
                            }
                        }
                    }
                    if p.x < CABECALHO_LINHA && p.y >= CABECALHO_COLUNA {
                        let linhas = self.linhas_visiveis(bounds.height/self.zoom).0;
                        if let Some(&(row, y, h)) = linhas.iter().find(|&&(_, y, h)|
                            (p.y - (y + h)).abs() <= FOLGA_DIVISORIA) {
                            return (Captured::Yes, clique_divisoria(state, false, row, y, y + h));
                        }
                        for &(row, y, h) in &linhas {
                            if p.y >= y && p.y < y+h {
                                if state.shift {
                                    return (Captured::Yes, Some(Msg::EstenderLinha(row)));
                                }
                                state.cabecalho = Some((false, row));
                                return (Captured::Yes, Some(Msg::SelecionarLinha(row)));
                            }
                        }
                    }
                    let cell = self.celula_no_ponto(point, bounds.size());
                    if let Some((r,c)) = cell {
                        // Ctrl+clique visita links reconhecidos sem iniciar
                        // arrasto da seleção no Canvas.
                        if self.ctrl_pressionado {
                            if let Some(aba) = self.workbook.aba(self.aba) {
                                let link = aba.sheet.estilo_resolvido(r,c)
                                    .and_then(|s| s.link)
                                    .and_then(|s| crate::app::link_ui::normalizar_link(&s))
                                    .or_else(|| crate::app::link_ui::normalizar_link(
                                        &self.texto(r,c)));
                                if link.is_some() {
                                    return (Captured::Yes,
                                        Some(Msg::LinkAbrirEm(r,c)));
                                }
                            }
                            // Ctrl+clique fora de link: mais uma área na seleção;
                            // arrastar em seguida estende a área nova.
                            state.celulas = true;
                            return (Captured::Yes, Some(Msg::SelecionarAdicional(r,c)));
                        }
                        // +/− da dinâmica é clicável na própria célula, sem
                        // abrir editor nem deslocar a seleção involuntariamente.
                        if let Some(a) = self.workbook.aba(self.aba) {
                            let mapa = crate::dinamica_visual::com_cache(&a.sheet,a.versao);
                            if let Some((caminho,_)) = crate::dinamica_visual::marca(&mapa,r,c) {
                                let mut esquerda = CABECALHO_LINHA;
                                for indice in self.primeira_coluna_visivel(bounds.width)..c {
                                    esquerda += largura(self.workbook,self.aba,indice);
                                }
                                if p.x >= esquerda+3.0 && p.x <= esquerda+23.0 {
                                    return (Captured::Yes,
                                        Some(Msg::Dinamica(crate::app::dinamica_ui::Evento::RecolherNaGrade(r,c,caminho))));
                                }
                            }
                        }
                        // Funil do filtro de relatório da dinâmica: a lista de itens.
                        if self.workbook.aba(self.aba).is_some_and(|a| crate::dinamica_visual::filtro_na_grade(
                            &crate::dinamica_visual::com_cache(&a.sheet,a.versao),r,c).is_some())
                        {
                            let (direita, baixo) = self.posicao_da_celula(r,c,
                                Size::new(bounds.width/self.zoom,bounds.height/self.zoom))
                                .map(|(x,y,w,h)| (x+w, y+h)).unwrap_or((0.0, 0.0));
                            if p.x >= direita-24.0 && p.x<=direita {
                                // A lista abre logo abaixo do funil (em pixels da tela).
                                return (Captured::Yes,
                                    Some(Msg::Dinamica(crate::app::dinamica_ui::Evento::FiltroNaGrade(r,c,
                                        (direita-24.0)*self.zoom, baixo*self.zoom))));
                            }
                        }
                        // Setinha no cabeçalho: abre o painel da própria coluna.
                        if self.workbook.aba(self.aba).is_some_and(|a|
                            crate::app::tabelas::tem_filtro_cabecalho(&a.sheet,r,c))
                        {
                            let direita = self.posicao_da_celula(r,c,
                                Size::new(bounds.width/self.zoom,bounds.height/self.zoom))
                                .map(|(x,_,w,_)| x+w).unwrap_or(0.0);
                            if p.x >= direita-24.0 && p.x<=direita {
                                return (Captured::Yes,
                                    Some(Msg::TabelaAcao(crate::app::tabelas::Acao::FiltroCabecalho(r,c,point.x,point.y))));
                            }
                        }
                        // Seta da célula de total: só vale com a célula já ativa,
                        // que é quando ela está desenhada.
                        if (r,c) == self.selecionada && self.workbook.aba(self.aba).is_some_and(|a|
                            crate::app::tabelas::e_celula_de_total(
                                &crate::app::tabelas::visuais(&a.sheet),r,c))
                        {
                            let direita = self.posicao_da_celula(r,c,
                                Size::new(bounds.width/self.zoom,bounds.height/self.zoom))
                                .map(|(x,_,w,_)| x+w).unwrap_or(0.0);
                            if p.x >= direita-24.0 && p.x<=direita {
                                return (Captured::Yes,
                                    Some(Msg::TabelaAcao(crate::app::tabelas::Acao::AbrirTotais(r,c,point.x,point.y))));
                            }
                        }
                        // Dois cliques consecutivos na mesma célula abrem o editor
                        // diretamente sobre ela (sem desviar a digitação para fx).
                        if clique_duplo_celula(state, r, c) {
                            state.celulas = false;
                            let viewport = Size::new(bounds.width / self.zoom, bounds.height / self.zoom);
                            let mescla = self.workbook.aba(self.aba).and_then(|a|
                                crate::app::mesclas::ler(&a.sheet).into_iter()
                                    .find(|m| m.contains(r, c)));
                            let (linha, coluna, fim_linha, fim_coluna) = mescla
                                .map(|m| (m.r0, m.c0, m.r1, m.c1))
                                .unwrap_or((r, c, r, c));
                            if let Some((x, y, w, h)) =
                                self.posicao_da_celula(linha, coluna, viewport)
                            {
                                // Usa apenas as faixas visíveis: mesclas grandes não
                                // percorrem milhões de linhas ou colunas.
                                let largura = self.colunas_visiveis(viewport.width).0.iter()
                                    .filter(|(c, _, _)| *c >= coluna && *c <= fim_coluna)
                                    .map(|(_, _, v)| *v).sum::<f32>().max(w);
                                let altura = self.linhas_visiveis(viewport.height).0.iter()
                                    .filter(|(r, _, _)| *r >= linha && *r <= fim_linha)
                                    .map(|(_, _, v)| *v).sum::<f32>().max(h);
                                return (Captured::Yes,
                                    Some(Msg::EditarNaCelula(linha, coluna,
                                        x * self.zoom, y * self.zoom,
                                        largura * self.zoom, altura * self.zoom)));
                            }
                        }
                        state.celulas = true;
                        return (Captured::Yes,Some(Msg::Selecionar(r,c)));
                    }
                }
            }
            Event::MouseReleased(mouse::Button::Left) => {
                if let Some(mut arraste) = state.referencia.take() {
                    if let Some(point) = local(cursor, bounds) {
                        if let Some(celula) = self.celula_no_ponto(point, bounds.size()) {
                            arraste.destino = celula;
                        }
                    }
                    if arraste.inicio != arraste.destino {
                        return (Captured::Yes, Some(Msg::ArrastarReferencia(
                            arraste.start, arraste.pegada, arraste.inicio, arraste.destino
                        )));
                    }
                    return (Captured::Yes, None);
                }
                state.cabecalho = None;
                if state.celulas {
                    state.celulas = false;
                    // App decide se o pincel está armado. Um único evento
                    // após o drag garante uma transação, inclusive por clique.
                    return (Captured::Yes, Some(Msg::AplicarPincel));
                }
            }
            Event::ModifiersChanged(m) => {
                state.shift = m.shift_key();
            }
            Event::CursorMoved(_) if state.referencia.is_some() => {
                if let Some(point) = local(cursor, bounds) {
                    if let Some(celula) = self.celula_no_ponto(point, bounds.size()) {
                        if let Some(arraste) = &mut state.referencia { arraste.destino = celula; }
                    }
                }
                return (Captured::Yes, None);
            }
            Event::CursorMoved(_) if state.cabecalho.is_some() => {
                if let (Some(p), Some((coluna, _))) = (local(cursor, bounds), state.cabecalho) {
                    let p = Point::new(p.x / self.zoom, p.y / self.zoom);
                    if coluna {
                        if let Some(&(col, _, _)) = self.colunas_visiveis(bounds.width/self.zoom).0
                            .iter().find(|&&(_, x, w)| p.x >= x && p.x < x + w) {
                            return (Captured::Yes, Some(Msg::EstenderColuna(col)));
                        }
                    } else if let Some(&(row, _, _)) = self.linhas_visiveis(bounds.height/self.zoom).0
                        .iter().find(|&&(_, y, h)| p.y >= y && p.y < y + h) {
                        return (Captured::Yes, Some(Msg::EstenderLinha(row)));
                    }
                }
            }
            Event::CursorMoved(_) if state.celulas => {
                if let Some(p) = local(cursor, bounds) {
                    if let Some((r,c)) = self.celula_no_ponto(p, bounds.size()) {
                        return (Captured::Yes, Some(Msg::Estender(r,c)));
                    }
                }
            }
            // Passar o mouse sobre célula comentada mostra o balão. Só avisa a
            // App quando a célula sob o mouse muda (nada por quadro).
            Event::CursorMoved(_) => {
                let sob = local(cursor, bounds)
                    .and_then(|p| self.comentario_no_ponto(p, bounds.size()));
                if sob != self.comentarios.sobre {
                    return (Captured::No, Some(Msg::Comentario(crate::app::EventoComentario::Sobre(sob))));
                }
            }
            Event::Wheel { dx, dy, .. } => {
                if let Some(ponto) = local(cursor, bounds) {
                    // Um clique da roda são 48 px no pelicano (o `Lines` de 1 do Iced).
                    let (x, y) = (dx / 48.0, dy / 48.0);
                    // Roda sobre a lista aberta rola a lista, não a planilha.
                    if self.lista_aberta {
                        if let Some(opcoes) = self.opcoes_lista() {
                            let viewport = Size::new(bounds.width / self.zoom, bounds.height / self.zoom);
                            let p = Point::new(ponto.x / self.zoom, ponto.y / self.zoom);
                            if self.item_da_lista(p, viewport, opcoes.len()).is_some() {
                                let passo = -passos_da_roda(&mut state.roda.0, y * 3.0) as i64;
                                let atual = lista_suspensa::inicio_valido(self.lista_pagina, opcoes.len()) as i64;
                                let novo = lista_suspensa::inicio_valido((atual + passo).max(0) as usize, opcoes.len());
                                return (Captured::Yes, (novo as i64 != atual)
                                    .then_some(Msg::ValidacaoListaPagina(novo as i32)));
                            }
                        }
                    }
                    // O Canvas captura a roda sobre a célula: enviar zoom antes do scroll.
                    if self.ctrl_pressionado && y.abs() >= 0.1 {
                        return (Captured::Yes, Some(Msg::Roda(y.signum() as i32)));
                    }
                    // Shift+roda rola na horizontal, como no Excel.
                    let (x, y) = if state.shift && x.abs() < 0.01 { (y, 0.0) } else { (x, y) };
                    let dr = passos_da_roda(&mut state.roda.0, y * 3.0);
                    let dc = passos_da_roda(&mut state.roda.1, x * 2.0);
                    if dr != 0 || dc != 0 {
                        // A roda usa a origem efetivamente visível e o teto
                        // derivado do zoom/altura real, sem inércia além da linha 10M.
                        let primeira_linha = self.origem_apos_rolar(bounds.height, -dr);
                        let no_rodape = primeira_linha == self.limite_inferior_rolagem(bounds.height);
                        return (Captured::Yes, Some(Msg::RolarNaGrade(
                            primeira_linha, self.primeira_coluna_visivel(bounds.width), -dc, no_rodape,
                        )));
                    }
                }
            }
            _ => {}
        }
        (Captured::No,None)
    }
}

/// Lado da caixa de seleção desenhada na célula (unidades da grade).
const LADO_CAIXA: f32 = 14.0;

/// A caixa de seleção centrada na célula: vazia com borda, ou cheia na cor
/// de destaque com o visto branco.
#[allow(clippy::too_many_arguments)]
fn desenhar_caixa(f: &mut Frame, x: f32, y: f32, w: f32, h: f32, marcada: bool, escuro: bool, ativo: Color) {
    let lado = LADO_CAIXA.min(w - 4.0).min(h - 4.0);
    if lado < 6.0 { return; }
    let canto = Point::new(x + (w - lado) / 2.0, y + (h - lado) / 2.0);
    let mut caixa = canvas::path::Builder::new();
    caixa.rounded_rectangle(canto, Size::new(lado, lado), 2.5);
    let caixa = caixa.build();
    if marcada {
        f.fill(&caixa, ativo);
        let s = lado / 14.0;
        let visto = canvas::Path::new(|p| {
            p.move_to(Point::new(canto.x + 3.2 * s, canto.y + 7.3 * s));
            p.line_to(Point::new(canto.x + 5.9 * s, canto.y + 10.0 * s));
            p.line_to(Point::new(canto.x + 10.9 * s, canto.y + 4.2 * s));
        });
        f.stroke(&visto, canvas::Stroke::default().with_color(Color::WHITE).with_width(1.8 * s)
            .with_line_cap(canvas::LineCap::Round).with_line_join(canvas::LineJoin::Round));
    } else {
        f.fill(&caixa, if escuro { Color::rgb8(37, 45, 58) } else { Color::WHITE });
        let borda = if escuro { Color::rgb8(150, 160, 175) } else { Color::rgb8(110, 118, 130) };
        f.stroke(&caixa, canvas::Stroke::default().with_color(borda).with_width(1.2));
    }
}

/// Detecta dois cliques rápidos na MESMA célula. Um clique normal não edita,
/// e arrastos/controles de tabela mantêm seus gestos próprios.
fn clique_duplo_celula(state: &mut EstadoMouse, linha: u32, coluna: u32) -> bool {
    let agora = std::time::Instant::now();
    let duplo = state.ultimo_clique_celula.is_some_and(|(instante, r, c)|
        r == linha && c == coluna &&
        agora.duration_since(instante) <= INTERVALO_DUPLO_CLIQUE);
    state.ultimo_clique_celula = if duplo { None } else { Some((agora, linha, coluna)) };
    duplo
}

#[cfg(test)]
#[test]
fn duplo_clique_exige_mesma_celula_e_reinicia_contagem() {
    let mut mouse = EstadoMouse::default();
    assert!(!clique_duplo_celula(&mut mouse, 5, 2));
    assert!(!clique_duplo_celula(&mut mouse, 5, 3));
    assert!(clique_duplo_celula(&mut mouse, 5, 3));
    assert!(!clique_duplo_celula(&mut mouse, 5, 3));
}

/// Registra o clique numa divisória: o segundo clique ajusta ao conteúdo; o
/// primeiro começa o arrasto que muda o tamanho.
fn clique_divisoria(state: &mut EstadoMouse, coluna: bool, indice: u32, inicio: f32, fim: f32) -> Option<Msg> {
    let agora = std::time::Instant::now();
    let duplo = state.divisoria.is_some_and(|(t, c, i)|
        c == coluna && i == indice && agora.duration_since(t) <= INTERVALO_DUPLO_CLIQUE);
    state.divisoria = if duplo { None } else { Some((agora, coluna, indice)) };
    state.dimensao = (!duplo).then_some(ArrastoDimensao { coluna, indice, inicio, atual: fim, original: (fim - inicio).round() as u32 });
    duplo.then_some(Msg::AjustarDivisoria(coluna, indice))
}

/// A linha-guia do arrasto e a medida ao lado, como a dica do Excel.
fn desenhar_guia_dimensao(f: &mut Frame, bounds: Rectangle, a: ArrastoDimensao, cor: Color, zoom: f32) {
    let e = 1.5 / zoom.max(0.25);
    let rotulo = trf!("{}: {} px", if a.coluna { tr!("Largura") } else { tr!("Altura") }, a.tamanho());
    let (caixa, guia) = if a.coluna {
        (Point::new(a.atual + 6.0, 2.0), (Point::new(a.atual - e / 2.0, 0.0), Size::new(e, bounds.height)))
    } else {
        (Point::new(CABECALHO_LINHA + 4.0, a.atual + 4.0), (Point::new(0.0, a.atual - e / 2.0), Size::new(bounds.width, e)))
    };
    f.fill_rectangle(guia.0, guia.1, cor);
    let largura = rotulo.chars().count() as f32 * 6.6 + 14.0;
    f.fill(&canvas::Path::rounded_rectangle(caixa, Size::new(largura, 20.0), 4.0), cor);
    f.fill_text(Text {
        content: rotulo,
        position: Point::new(caixa.x + 7.0, caixa.y + 10.0),
        color: Color::WHITE,
        size: crate::compat::Pixels(11.5),
        vertical_alignment: pelicano::Align::Center,
        ..Text::default()
    });
}

#[cfg(test)]
mod testes;
