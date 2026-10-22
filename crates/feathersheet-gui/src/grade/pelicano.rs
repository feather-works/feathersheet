//! Canvas Pelicano com a mesma geometria esparsa usada na grade Iced.
use super::*;
use ::pelicano as p;
use p::Program;

#[derive(Default)]
pub struct Mouse {
    arrastando: bool,
    shift: bool,
    ultimo: Option<(std::time::Instant, u32, u32)>,
    roda_x: f32,
    roda_y: f32,
    barra: Option<barras::ArrastoBarra>,
}

fn caixa(x: f32, y: f32, w: f32, h: f32) -> p::Rect {
    p::Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

fn fundo(r: &mut p::Renderer, rect: p::Rect, cor: p::Color) {
    r.fill_quad(
        rect,
        Some(&p::Background::Color(cor)),
        p::Border::default(),
        None,
    );
}

fn cor(c: iced::Color) -> p::Color {
    p::Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

impl Program<Msg> for Grade<'_> {
    type State = Mouse;

    fn draw(
        &self,
        _: &Mouse,
        r: &mut p::Renderer,
        bounds: p::Rect,
        tema: &p::Theme,
        _: Option<p::Point>,
    ) {
        self.eixos.borrow_mut().tamanho_canvas = Some(iced::Size::new(bounds.width, bounds.height));
        let z = self.zoom;
        let (linhas, _) = self.linhas_visiveis(bounds.height / z);
        let (colunas, _) = self.colunas_visiveis(bounds.width / z);
        let (alturas, larguras) = self.extensoes_inteiras(
            &linhas,
            &colunas,
            iced::Size::new(bounds.width / z, bounds.height / z),
        );
        r.push_clip(bounds, [0.; 4]);
        fundo(r, bounds, tema.surface);
        let tabelas = self
            .workbook
            .aba(self.aba)
            .map(|a| crate::app::tabelas::visuais(&a.sheet))
            .unwrap_or_default();
        for (ci, &(c, x, w)) in colunas.iter().enumerate() {
            for (ri, &(l, y, h)) in linhas.iter().enumerate() {
                let rect = caixa(bounds.x + x * z, bounds.y + y * z, w * z, h * z);
                let estilo = self
                    .workbook
                    .aba(self.aba)
                    .and_then(|a| a.sheet.estilo_resolvido(l, c));
                let cond = self
                    .condicional
                    .borrow_mut()
                    .formato(self.workbook, self.aba, l, c);
                let visual = crate::estilos_comuns::projetar_estilo(
                    estilo.as_ref(),
                    cond.as_ref().and_then(|v| v.style.as_ref()),
                );
                if let Some([red, green, blue, alpha]) = visual.comum.basico.fundo {
                    fundo(
                        r,
                        rect,
                        p::Color::rgb8(red, green, blue).alpha(alpha as f32 / 255.),
                    );
                } else if let Some(bg) = crate::app::tabelas::fundo(&tabelas, l, c, self.escuro) {
                    fundo(r, rect, cor(bg));
                }
                if self.area.contains(l, c) {
                    fundo(
                        r,
                        rect,
                        tema.primary.alpha(if (l, c) == self.selecionada {
                            0.06
                        } else {
                            0.14
                        }),
                    );
                }
                if self.linhas_grade {
                    r.stroke(
                        &[
                            p::Point::new(rect.x, rect.y + rect.height),
                            p::Point::new(rect.x + rect.width, rect.y + rect.height),
                        ],
                        0.65,
                        tema.border,
                    );
                    r.stroke(
                        &[
                            p::Point::new(rect.x + rect.width, rect.y),
                            p::Point::new(rect.x + rect.width, rect.y + rect.height),
                        ],
                        0.65,
                        tema.border,
                    );
                }
                let valor = if self.workbook.calculo_pendente(self.aba, l, c) {
                    feathersheet_core::celulas::values::Value::Empty
                } else {
                    self.workbook.valor(self.aba, l, c)
                };
                let texto = self.texto(l, c);
                if !texto.is_empty() {
                    let tamanho = estilo
                        .as_ref()
                        .and_then(|s| s.font_size)
                        .map_or(self.tamanho_fonte_padrao as f32, |s| s as f32)
                        * z;
                    let fonte = p::Font {
                        weight: if visual.comum.basico.negrito {
                            700
                        } else {
                            400
                        },
                        italic: visual.comum.basico.italico,
                        ..p::Font::DEFAULT
                    };
                    let cor_texto = visual
                        .comum
                        .basico
                        .texto
                        .map(|[r, g, b, a]| p::Color::rgb8(r, g, b).alpha(a as f32 / 255.))
                        .unwrap_or(tema.text);
                    let medido = r.measure(&texto, tamanho, fonte, None);
                    let largura = larguras[ci] * z;
                    let pos_x = match effective_align(&valor, estilo.as_ref()) {
                        Align::Right => rect.x + largura - medido.width - 7. * z,
                        Align::Center => rect.x + (largura - medido.width) / 2.,
                        _ => rect.x + 7. * z,
                    };
                    r.push_clip(rect, [0.; 4]);
                    r.draw_text(
                        &texto,
                        tamanho,
                        fonte,
                        cor_texto,
                        p::Point::new(
                            pos_x,
                            rect.y + ((alturas[ri] * z - medido.height) / 2.).max(1.),
                        ),
                        None,
                    );
                    for (ativo, y) in [
                        (
                            visual.comum.basico.sublinhado,
                            rect.y + (alturas[ri] * z + medido.height) / 2. - 1.,
                        ),
                        (visual.riscado, rect.y + alturas[ri] * z / 2.),
                    ] {
                        if ativo {
                            r.stroke(
                                &[
                                    p::Point::new(pos_x, y),
                                    p::Point::new(pos_x + medido.width, y),
                                ],
                                z.max(1.),
                                cor_texto,
                            );
                        }
                    }
                    r.pop_clip();
                }
                if (l, c) == self.selecionada {
                    r.fill_quad(
                        rect,
                        None,
                        p::Border::default().color(tema.primary, 2.),
                        None,
                    );
                    fundo(
                        r,
                        caixa(rect.x + rect.width - 3., rect.y + rect.height - 3., 5., 5.),
                        tema.primary,
                    );
                }
            }
        }
        let header = tema.background;
        fundo(
            r,
            caixa(bounds.x, bounds.y, bounds.width, CABECALHO_COLUNA * z),
            header,
        );
        fundo(
            r,
            caixa(bounds.x, bounds.y, CABECALHO_LINHA * z, bounds.height),
            header,
        );
        for &(c, x, w) in &colunas {
            let rect = caixa(bounds.x + x * z, bounds.y, w * z, CABECALHO_COLUNA * z);
            if c >= self.area.c0 && c <= self.area.c1 {
                fundo(r, rect, tema.primary.alpha(0.13));
            }
            let texto = col_name(c);
            let medida = r.measure(&texto, 12. * z, p::Font::MEDIUM, None);
            r.draw_text(
                &texto,
                12. * z,
                p::Font::MEDIUM,
                tema.muted,
                p::Point::new(
                    rect.x + (rect.width - medida.width) / 2.,
                    rect.y + (rect.height - medida.height) / 2.,
                ),
                None,
            );
        }
        for &(l, y, h) in &linhas {
            let rect = caixa(bounds.x, bounds.y + y * z, CABECALHO_LINHA * z, h * z);
            if l >= self.area.r0 && l <= self.area.r1 {
                fundo(r, rect, tema.primary.alpha(0.13));
            }
            let texto = (l + 1).to_string();
            let medida = r.measure(&texto, 12. * z, p::Font::DEFAULT, None);
            r.draw_text(
                &texto,
                12. * z,
                p::Font::DEFAULT,
                tema.muted,
                p::Point::new(
                    rect.x + rect.width - medida.width - 12.,
                    rect.y + (rect.height - medida.height) / 2.,
                ),
                None,
            );
        }
        self.desenhar_barras_pelicano(r, bounds, tema);
        r.pop_clip();
    }

    fn update(
        &self,
        state: &mut Mouse,
        ev: &p::Event,
        bounds: p::Rect,
        cursor: Option<p::Point>,
        publish: &mut dyn FnMut(Msg),
    ) -> p::Captured {
        if let p::Event::ModifiersChanged(m) = ev {
            state.shift = m.shift_key();
        }
        if matches!(ev, p::Event::MouseReleased(p::MouseButton::Left)) {
            state.arrastando = false;
            state.barra = None;
        }
        if let (Some(arrasto), p::Event::CursorMoved(p)) = (state.barra, ev) {
            publish(self.arrastar_barra(arrasto, iced::Point::new(p.x - bounds.x, p.y - bounds.y)));
            return p::Captured::Yes;
        }
        let Some(p) = cursor.filter(|p| {
            p.x >= bounds.x
                && p.y >= bounds.y
                && p.x < bounds.x + bounds.width
                && p.y < bounds.y + bounds.height
        }) else {
            return p::Captured::No;
        };
        let tamanho = iced::Size::new(bounds.width / self.zoom, bounds.height / self.zoom);
        let local = iced::Point::new((p.x - bounds.x) / self.zoom, (p.y - bounds.y) / self.zoom);
        match ev {
            p::Event::MousePressed(p::MouseButton::Left) => {
                if let Some((arrasto, msg)) = self.pressionar_barra(
                    iced::Point::new(p.x - bounds.x, p.y - bounds.y),
                    iced::Size::new(bounds.width, bounds.height),
                ) {
                    state.barra = arrasto;
                    if let Some(msg) = msg {
                        publish(msg);
                    }
                    return p::Captured::Yes;
                }
                if let Some((l, c)) = self.ponto_para_celula(local, tamanho) {
                    state.arrastando = true;
                    let agora = std::time::Instant::now();
                    let duplo = state.ultimo.is_some_and(|(t, r, k)| {
                        (r, k) == (l, c)
                            && agora.duration_since(t) < std::time::Duration::from_millis(450)
                    });
                    state.ultimo = Some((agora, l, c));
                    publish(if state.shift {
                        Msg::Estender(l, c)
                    } else {
                        Msg::Selecionar(l, c)
                    });
                    if duplo {
                        publish(Msg::Digitar(
                            self.workbook
                                .aba(self.aba)
                                .map(|a| a.sheet.get_raw(l, c))
                                .unwrap_or_default(),
                        ));
                    }
                } else if local.y < CABECALHO_COLUNA && local.x >= CABECALHO_LINHA {
                    if let Some(&(c, _, _)) = self
                        .colunas_visiveis(tamanho.width)
                        .0
                        .iter()
                        .find(|(_, x, w)| local.x >= *x && local.x < *x + *w)
                    {
                        publish(Msg::SelecionarColuna(c));
                    }
                } else if local.x < CABECALHO_LINHA && local.y >= CABECALHO_COLUNA {
                    if let Some(&(l, _, _)) = self
                        .linhas_visiveis(tamanho.height)
                        .0
                        .iter()
                        .find(|(_, y, h)| local.y >= *y && local.y < *y + *h)
                    {
                        publish(Msg::SelecionarLinha(l));
                    }
                } else {
                    publish(Msg::SelecionarTudo);
                }
            }
            p::Event::CursorMoved(_) if state.arrastando => {
                if let Some((l, c)) = self.ponto_para_celula(local, tamanho) {
                    publish(Msg::Estender(l, c));
                }
            }
            p::Event::Wheel { dx, dy, modifiers } => {
                if modifiers.control_key() {
                    publish(Msg::Zoom(
                        ((self.zoom * 100.) as i32 + if *dy > 0. { 10 } else { -10 }).clamp(25, 400)
                            as u16,
                    ));
                } else {
                    state.roda_y -= dy;
                    state.roda_x -= dx;
                    let linhas = (state.roda_y / (ALTURA_PADRAO * self.zoom)).trunc() as i32;
                    let colunas = (state.roda_x / (LARGURA_PADRAO * self.zoom)).trunc() as i32;
                    state.roda_y -= linhas as f32 * ALTURA_PADRAO * self.zoom;
                    state.roda_x -= colunas as f32 * LARGURA_PADRAO * self.zoom;
                    if linhas != 0 || colunas != 0 {
                        publish(Msg::Rolar(linhas, colunas));
                    }
                }
            }
            _ => return p::Captured::No,
        }
        p::Captured::Yes
    }
}
