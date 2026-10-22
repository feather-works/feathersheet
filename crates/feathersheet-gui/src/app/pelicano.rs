//! Interface Pelicano. Estado, validação, histórico e arquivos pertencem ao App existente.
use super::{App as Planilha, Msg as Comando, *};
use ::pelicano as p;
use p::{column, row, App as _, Element, Length, Task};

mod tarefas;

#[derive(Clone)]
pub(crate) enum Msg {
    Comando(Comando),
    Tarefa(Arc<Mutex<Option<iced_runtime::Action<Comando>>>>),
    Faixa(usize),
    Tema,
    Pulso,
    Focar,
    Janela(p::WindowCmd),
    Colado(Option<String>),
}

pub(crate) struct Interface {
    app: Planilha,
    faixa: usize,
    foco: u64,
}

fn comando(msg: Comando) -> Msg {
    Msg::Comando(msg)
}

fn botao(rotulo: &str, msg: Comando) -> Element<'_, Msg> {
    let primario = matches!(
        msg,
        Comando::Salvar | Comando::SalvarAntes | Comando::Confirmar
    );
    p::button(p::text(rotulo).size(13))
        .padding([7, 11])
        .on_press(comando(msg))
        .style(move |t, s| {
            if primario {
                p::Style::default()
                    .background(if s == p::Status::Hovered {
                        t.primary.mix(t.text, 0.1)
                    } else {
                        t.primary
                    })
                    .text_color(t.on_primary)
                    .border(p::Border::rounded(6.))
                    .transition(p::Transition::new(120))
            } else {
                estilo_botao(t, s)
            }
        })
        .into()
}

fn estilo_botao(t: &p::Theme, s: p::Status) -> p::Style {
    p::Style::default()
        .background(match s {
            p::Status::Pressed => t.primary.alpha(0.16),
            p::Status::Hovered => t.primary.alpha(0.08),
            _ => t.surface,
        })
        .text_color(t.text)
        .border(p::Border::rounded(6.).color(t.border.alpha(0.8), 1.))
        .transition(p::Transition::new(120))
}

fn painel(tema: &p::Theme) -> p::Style {
    p::Style::default()
        .background(tema.surface)
        .border(p::Border::rounded(10.).color(tema.border, 1.))
}

fn redimensionavel(conteudo: Element<'_, Msg>) -> Element<'_, Msg> {
    use p::{Align, CursorIcon as C, ResizeDirection as D};
    let alca = |w, h, d, c, x, y| {
        p::container(
            p::mouse_area(p::Space::new(w, h))
                .on_press(Msg::Janela(p::WindowCmd::DragResize(d)))
                .cursor_icon(c),
        )
        .width(Length::FILL)
        .height(Length::FILL)
        .align_x(x)
        .align_y(y)
    };
    p::stack![
        conteudo,
        alca(
            Length::Fixed(5.),
            Length::FILL,
            D::West,
            C::WResize,
            Align::Start,
            Align::Start
        ),
        alca(
            Length::Fixed(5.),
            Length::FILL,
            D::East,
            C::EResize,
            Align::End,
            Align::Start
        ),
        alca(
            Length::FILL,
            Length::Fixed(5.),
            D::North,
            C::NResize,
            Align::Start,
            Align::Start
        ),
        alca(
            Length::FILL,
            Length::Fixed(5.),
            D::South,
            C::SResize,
            Align::Start,
            Align::End
        ),
        alca(
            Length::Fixed(12.),
            Length::Fixed(12.),
            D::NorthWest,
            C::NwResize,
            Align::Start,
            Align::Start
        ),
        alca(
            Length::Fixed(12.),
            Length::Fixed(12.),
            D::NorthEast,
            C::NeResize,
            Align::End,
            Align::Start
        ),
        alca(
            Length::Fixed(12.),
            Length::Fixed(12.),
            D::SouthWest,
            C::SwResize,
            Align::Start,
            Align::End
        ),
        alca(
            Length::Fixed(12.),
            Length::Fixed(12.),
            D::SouthEast,
            C::SeResize,
            Align::End,
            Align::End
        ),
    ]
    .into()
}

/// O campo confirma ou cancela a edição antes de devolver as teclas à grade.
struct CampoTeclado<'a> {
    conteudo: Element<'a, Msg>,
    manter_foco: bool,
}

impl p::Widget<Msg> for CampoTeclado<'_> {
    fn size(&self) -> (Length, Length) {
        self.conteudo.widget().size()
    }
    fn layout(&self, cx: &mut p::LayoutCx, l: p::Limits) -> p::Node {
        self.conteudo.widget().layout(cx, l)
    }
    fn draw(&self, n: &p::Node, cx: &mut p::DrawCx) {
        self.conteudo.widget().draw(n, cx);
    }
    fn event(&self, n: &p::Node, e: &p::Event, cx: &mut p::EventCx<Msg>) -> p::Captured {
        if cx.states.focus == Some(n.key) {
            if let p::Event::Key {
                key: p::Key::Named(k),
                modifiers,
                pressed: true,
                ..
            } = e
            {
                match k {
                    p::NamedKey::Escape => {
                        cx.states.focus = None;
                        cx.publish(comando(Comando::Cancelar));
                        return p::Captured::Yes;
                    }
                    p::NamedKey::Tab => {
                        cx.states.focus = None;
                        cx.publish(comando(Comando::Confirmar));
                        cx.publish(comando(Comando::Mover(
                            0,
                            if modifiers.shift_key() { -1 } else { 1 },
                        )));
                        return p::Captured::Yes;
                    }
                    p::NamedKey::Enter => {
                        let capturado = self.conteudo.widget().event(n, e, cx);
                        if !self.manter_foco {
                            cx.states.focus = None;
                        }
                        return capturado;
                    }
                    _ => {}
                }
            }
        }
        self.conteudo.widget().event(n, e, cx)
    }
}

impl Interface {
    fn nova(inicial: Option<PathBuf>) -> (Self, Task<Msg>) {
        let config = Configuracao::carregar();
        let mut app = Planilha {
            config,
            em_tela_inicio: false,
            ..Default::default()
        };
        app.status = "Pronto".into();
        app.janelas.principal = Some(window::Id::unique());
        app.workbook = Planilha::pasta_nova();
        app.nome_aba = app.workbook.nomes()[0].clone();
        app.linhas_grade = app.config.linhas_grade;
        app.mostrar_fx = app.config.barra_formulas;
        app.mostrar_formulas = app.config.mostrar_formulas;
        feathersheet_core::set_locale_config(app.regionalizacao());
        definir_tema(
            escuro_resolvido(app.config.tema_auto, app.config.escuro),
            app.config.acento,
        );
        feathersheet_io::memoria::definir_limite_gb(app.config.limite_memoria_gb);
        feathersheet_core::paralelismo::definir(app.config.nucleos as usize);
        let t = inicial.map_or_else(iced::Task::none, |path| {
            app.update(Comando::ArquivoEscolhido(Some(path)))
        });
        (
            Self {
                app,
                faixa: 0,
                foco: 0,
            },
            tarefas::converter(t),
        )
    }

    fn executar(&mut self, msg: Comando) -> Task<Msg> {
        if self.app.carregando
            && !matches!(
                msg,
                Comando::FecharAplicativo | Comando::JanelaRedimensionada(..)
            )
        {
            return Task::none();
        }
        // Um diálogo modal nunca deixa comandos da faixa ou da grade alterarem a pasta por baixo.
        if self.app.pendente.is_some()
            && !matches!(
                msg,
                Comando::SalvarAntes
                    | Comando::Descartar
                    | Comando::Cancelar
                    | Comando::DestinoSalvar(_)
                    | Comando::Gravado(..)
            )
        {
            return Task::none();
        }
        if self.app.validacao_alerta.is_some()
            && !matches!(
                msg,
                Comando::ValidacaoEv(validacao_dialogo::Evento::Responder(_))
            )
        {
            return Task::none();
        }
        let focar = matches!(msg, Comando::Digitar(_) | Comando::AceitarSugestao(_));
        if focar {
            self.foco = self.foco.wrapping_add(1);
        }
        let tarefa = tarefas::converter(self.app.update(msg));
        if focar {
            Task::batch([tarefa, Task::focus("feathersheet.fx")])
        } else {
            tarefa
        }
    }

    fn faixa(&self) -> Element<'_, Msg> {
        let a = &self.app;
        let conteudo = match self.faixa {
            0 => row![
                self.grupo(
                    "Área de transferência",
                    vec![
                        botao("Colar", Comando::ColarSolicitar),
                        botao("Copiar", Comando::Copiar),
                        botao("Recortar", Comando::Recortar)
                    ]
                ),
                self.grupo(
                    "Fonte",
                    vec![
                        botao("N", Comando::AlternarEstilo("bold".into())),
                        botao("I", Comando::AlternarEstilo("italic".into())),
                        botao("S", Comando::AlternarEstilo("underline".into())),
                        botao("A−", Comando::TamanhoFonteDelta(-1)),
                        botao("A+", Comando::TamanhoFonteDelta(1))
                    ]
                ),
                self.grupo(
                    "Número",
                    vec![
                        botao("Geral", Comando::EstiloMenu("format".into(), Campo::Nulo)),
                        botao(
                            "R$",
                            Comando::EstiloMenu("format".into(), Campo::Texto("currency".into()))
                        ),
                        botao(
                            "%",
                            Comando::EstiloMenu("format".into(), Campo::Texto("percent".into()))
                        )
                    ]
                ),
                self.grupo(
                    "Alinhamento",
                    vec![
                        botao(
                            "Esquerda",
                            Comando::EstiloMenu("align".into(), Campo::Texto("left".into()))
                        ),
                        botao(
                            "Centro",
                            Comando::EstiloMenu("align".into(), Campo::Texto("center".into()))
                        ),
                        botao(
                            "Direita",
                            Comando::EstiloMenu("align".into(), Campo::Texto("right".into()))
                        )
                    ]
                )
            ],
            1 => row![
                self.grupo(
                    "Pasta de trabalho",
                    vec![
                        botao("Nova aba", Comando::AdicionarAba),
                        botao("Duplicar aba", Comando::DuplicarAba)
                    ]
                ),
                self.grupo(
                    "Células",
                    vec![
                        botao("Inserir linha", Comando::Estrutura(Estrutura::InserirLinha)),
                        botao(
                            "Inserir coluna",
                            Comando::Estrutura(Estrutura::InserirColuna)
                        )
                    ]
                ),
            ],
            2 => row![
                self.grupo(
                    "Cálculo",
                    vec![
                        botao("∑ AutoSoma", Comando::AutoSoma),
                        botao("Média", Comando::AutoConta("MÉDIA"))
                    ]
                ),
                self.grupo(
                    "Exibição",
                    vec![botao(
                        if a.mostrar_formulas {
                            "Mostrar valores"
                        } else {
                            "Mostrar fórmulas"
                        },
                        Comando::AlternarMostrarFormulas
                    )]
                ),
            ],
            3 => row![
                self.grupo(
                    "Ordenar",
                    vec![
                        botao("A → Z", Comando::Ordenar(true)),
                        botao("Z → A", Comando::Ordenar(false))
                    ]
                ),
                self.grupo(
                    "Limpeza",
                    vec![
                        botao("Aparar espaços", Comando::ApararEspacos),
                        botao("Limpar conteúdo", Comando::Limpar)
                    ]
                ),
            ],
            4 => row![
                self.grupo(
                    "Planilha",
                    vec![
                        botao(
                            if a.linhas_grade {
                                "Ocultar grade"
                            } else {
                                "Mostrar grade"
                            },
                            Comando::LinhasGrade(!a.linhas_grade)
                        ),
                        botao(
                            if a.mostrar_fx {
                                "Ocultar fórmulas"
                            } else {
                                "Barra de fórmulas"
                            },
                            Comando::AlternarBarraFx
                        )
                    ]
                ),
                self.grupo(
                    "Painéis",
                    vec![
                        botao("Fixar primeira linha", Comando::CongelarAplicar(1)),
                        botao("Fixar primeira coluna", Comando::CongelarAplicar(2)),
                        botao("Descongelar", Comando::CongelarAplicar(3))
                    ]
                ),
            ],
            _ => row![
                self.grupo(
                    "Documento",
                    vec![
                        botao("Novo", Comando::Novo),
                        botao("Abrir", Comando::Abrir),
                        botao("Salvar", Comando::Salvar),
                        botao("Salvar como…", Comando::SalvarComo)
                    ]
                ),
                self.grupo(
                    "Exportar",
                    vec![
                        botao("Excel", Comando::ExportarXlsx),
                        botao("CSV", Comando::ExportarDelimitado(false)),
                        botao("TSV", Comando::ExportarDelimitado(true)),
                        botao("JSON", Comando::ExportarJson)
                    ]
                ),
            ],
        }
        .spacing(10)
        .align(p::Align::Center);
        p::scrollable(conteudo)
            .direction(p::Direction::Horizontal)
            .height(89.)
            .into()
    }

    fn grupo<'a>(&self, titulo: &'a str, botoes: Vec<Element<'a, Msg>>) -> Element<'a, Msg> {
        p::container(
            column![p::row(botoes).spacing(4), p::text(titulo).size(11)]
                .spacing(9)
                .padding([10, 12]),
        )
        .style(painel)
        .into()
    }

    fn modal<'a>(
        &'a self,
        titulo: &'a str,
        texto: &'a str,
        acoes: Vec<Element<'a, Msg>>,
    ) -> Element<'a, Msg> {
        let caixa = p::container(
            column![
                p::text(titulo).size(21).font(p::Font::SEMIBOLD),
                p::text(texto).size(14),
                p::row(acoes).spacing(8)
            ]
            .spacing(20),
        )
        .padding(26)
        .width(530.)
        .style(painel);
        p::opaque(
            p::container(caixa)
                .width(Length::FILL)
                .height(Length::FILL)
                .align_x(p::Align::Center)
                .align_y(p::Align::Center)
                .style(|_| p::Style::default().background(p::Color::BLACK.alpha(0.45))),
        )
        .into()
    }
}

impl p::App for Interface {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Comando(Comando::ColarSolicitar) => Task::read_clipboard(Msg::Colado),
            Msg::Colado(Some(t)) => self.executar(Comando::Colar(t)),
            Msg::Colado(None) => Task::none(),
            Msg::Comando(msg) => self.executar(msg),
            Msg::Tarefa(acao) => acao
                .lock()
                .ok()
                .and_then(|mut a| a.take())
                .map_or_else(Task::none, |a| self.acao(a)),
            Msg::Faixa(i) => {
                self.faixa = i;
                Task::none()
            }
            Msg::Tema => self.executar(Comando::Tema(!escuro_resolvido(
                self.app.config.tema_auto,
                self.app.config.escuro,
            ))),
            Msg::Janela(cmd) => Task::window(cmd),
            Msg::Focar => self.executar(Comando::Digitar(self.app.edicao.clone())),
            Msg::Pulso => {
                let mut tarefas = Vec::new();
                let resumo = self.app.resumo_selecao.borrow().pendente;
                for (ativo, cmd) in [
                    (self.app.calculo_pendente, Comando::CalcularPulso),
                    (self.app.carregando, Comando::ProgressoPulso),
                    (resumo, Comando::ResumoPulso),
                ] {
                    if ativo {
                        tarefas.push(tarefas::converter(self.app.update(cmd)));
                    }
                }
                Task::batch(tarefas)
            }
        }
    }

    fn theme(&self) -> p::Theme {
        let escuro = escuro_resolvido(self.app.config.tema_auto, self.app.config.escuro);
        let mut tema = if escuro {
            p::Theme::dark()
        } else {
            p::Theme::light()
        };
        tema.primary = p::Color::rgb8(24, 153, 106);
        tema
    }

    fn subscription(&self) -> p::Subscription<Msg> {
        let pulso = if self.app.calculo_pendente
            || self.app.carregando
            || self.app.resumo_selecao.borrow().pendente
        {
            p::Subscription::<std::time::Instant>::every(Duration::from_millis(16))
                .map(|_| Msg::Pulso)
        } else {
            p::Subscription::none()
        };
        p::Subscription::batch([
            pulso,
            p::Subscription::<std::time::Instant>::every(Duration::from_secs(1))
                .map(|_| comando(Comando::MonitorPulso)),
        ])
    }

    fn on_event(&self, ev: &p::Event) -> Option<Msg> {
        match ev {
            p::Event::CloseRequested => Some(comando(Comando::FecharAplicativo)),
            p::Event::Resized(s) => self.app.janelas.principal.map(|id| {
                comando(Comando::JanelaRedimensionada(
                    id,
                    iced::Size::new(s.width, s.height),
                ))
            }),
            p::Event::Key {
                key: p::Key::Character(k),
                modifiers,
                pressed: true,
                ..
            } if modifiers.control_key() => match k.to_lowercase().as_str() {
                "s" => Some(comando(if modifiers.shift_key() {
                    Comando::SalvarComo
                } else {
                    Comando::Salvar
                })),
                "o" => Some(comando(Comando::Abrir)),
                "n" => Some(comando(Comando::Novo)),
                _ => None,
            },
            _ => None,
        }
    }

    fn on_unhandled_event(&self, _: p::WindowId, ev: &p::Event) -> Option<Msg> {
        if self.app.carregando || self.app.pendente.is_some() || self.app.validacao_alerta.is_some()
        {
            return None;
        }
        let p::Event::Key {
            key,
            text,
            modifiers,
            pressed: true,
        } = ev
        else {
            return None;
        };
        if modifiers.control_key() {
            if let p::Key::Character(k) = key {
                return match k.to_lowercase().as_str() {
                    "z" => Some(comando(if modifiers.shift_key() {
                        Comando::Refazer
                    } else {
                        Comando::Desfazer
                    })),
                    "y" => Some(comando(Comando::Refazer)),
                    "c" => Some(comando(Comando::Copiar)),
                    "x" => Some(comando(Comando::Recortar)),
                    "v" => Some(comando(Comando::ColarSolicitar)),
                    "a" => Some(comando(Comando::SelecionarTudo)),
                    _ => None,
                };
            }
        }
        if let p::Key::Named(k) = key {
            let cmd = match k {
                p::NamedKey::ArrowUp => Comando::Mover(-1, 0),
                p::NamedKey::ArrowDown => Comando::Mover(1, 0),
                p::NamedKey::ArrowLeft => Comando::Mover(0, -1),
                p::NamedKey::ArrowRight => Comando::Mover(0, 1),
                p::NamedKey::Tab => Comando::Mover(0, if modifiers.shift_key() { -1 } else { 1 }),
                p::NamedKey::Enter => Comando::EnviarEditor,
                p::NamedKey::Escape => Comando::Cancelar,
                p::NamedKey::Delete | p::NamedKey::Backspace => Comando::Limpar,
                p::NamedKey::PageDown => Comando::LinhaPagina(1),
                p::NamedKey::PageUp => Comando::LinhaPagina(-1),
                p::NamedKey::F2 => return Some(Msg::Focar),
                p::NamedKey::F11 => return Some(Msg::Janela(p::WindowCmd::ToggleFullscreen)),
                _ => return None,
            };
            return Some(comando(cmd));
        }
        if !modifiers.control_key() && !modifiers.alt_key() {
            if let Some(t) = text.as_ref().filter(|s| !s.chars().any(char::is_control)) {
                return Some(comando(Comando::Digitar(t.clone())));
            }
        }
        None
    }

    fn view(&self) -> Element<'_, Msg> {
        let a = &self.app;
        let tema = self.theme();
        static LOGO: std::sync::OnceLock<Option<p::Image>> = std::sync::OnceLock::new();
        let marca: Element<'_, Msg> = LOGO
            .get_or_init(|| {
                p::Image::from_png(include_bytes!(
                    "../../../../pacote/icones/feathersheet-256.png"
                ))
            })
            .as_ref()
            .map(|i| p::image(i.clone()).width(30.).height(30.).into())
            .unwrap_or_else(|| p::text("▦").size(25).color(tema.primary).into());
        let titulo = p::mouse_area(
            p::container(
                row![
                    marca,
                    column![
                        p::text("FeatherSheet").size(14).font(p::Font::SEMIBOLD),
                        p::text(format!(
                            "{}{}",
                            a.nome_documento(),
                            if a.alterado { " •" } else { "" }
                        ))
                        .size(11)
                        .color(tema.muted)
                    ]
                    .spacing(2),
                    p::Space::new(Length::FILL, 0.),
                ]
                .spacing(12)
                .align(p::Align::Center),
            )
            .padding([10, 16])
            .width(Length::FILL),
        )
        .on_press(Msg::Janela(p::WindowCmd::Drag));
        let topo = row![
            titulo,
            botao("Novo", Comando::Novo),
            botao("Abrir", Comando::Abrir),
            botao("Salvar", Comando::Salvar),
            Element::from(
                p::button(p::text("◐").size(18))
                    .on_press(Msg::Tema)
                    .style(estilo_botao)
            )
            .access_label("Alternar tema"),
            Element::from(
                p::button("−")
                    .on_press(Msg::Janela(p::WindowCmd::Minimize))
                    .style(estilo_botao)
            )
            .access_label("Minimizar"),
            Element::from(
                p::button("□")
                    .on_press(Msg::Janela(p::WindowCmd::ToggleMaximize))
                    .style(estilo_botao)
            )
            .access_label("Maximizar"),
            botao("×", Comando::FecharAplicativo),
        ]
        .spacing(6)
        .padding([0, 10])
        .align(p::Align::Center);
        let mut abas = p::row(Vec::<Element<'_, Msg>>::new()).spacing(4);
        for (i, nome) in [
            (5, "Arquivo"),
            (0, "Página Inicial"),
            (1, "Inserir"),
            (2, "Fórmulas"),
            (3, "Dados"),
            (4, "Exibir"),
        ] {
            abas = abas.push(
                p::button(p::text(nome).size(13))
                    .padding([8, 14])
                    .on_press(Msg::Faixa(i))
                    .style(move |t, status| {
                        p::Style::default()
                            .background(if i == self.faixa {
                                t.primary.alpha(0.14)
                            } else if status == p::Status::Hovered {
                                t.border.alpha(0.4)
                            } else {
                                p::Color::TRANSPARENT
                            })
                            .text_color(if i == self.faixa { t.primary } else { t.text })
                            .border(p::Border::rounded(7.))
                    }),
            );
        }
        abas = abas
            .push(p::Space::new(Length::FILL, 0.))
            .push(botao("Desfazer", Comando::Desfazer))
            .push(botao("Refazer", Comando::Refazer));
        let mut corpo = column![
            topo,
            p::container(abas).padding([4, 12]),
            p::container(self.faixa()).padding([4, 12])
        ]
        .spacing(1);
        if a.mostrar_fx || a.editor_formula.ativo {
            let campo = p::text_input("Valor ou fórmula", &a.edicao)
                .id("feathersheet.fx")
                .on_input(|t| comando(Comando::Editar(t)))
                .on_submit(comando(Comando::EnviarEditor))
                .padding([7, 12])
                .width(Length::FILL);
            let campo = if a.editor_formula.ativo {
                campo.autofocus(self.foco)
            } else {
                campo
            };
            corpo = corpo.push(
                p::container(
                    row![
                        Element::new(CampoTeclado {
                            manter_foco: false,
                            conteudo: p::text_input("A1", &a.endereco)
                                .width(102.)
                                .padding([7, 10])
                                .on_input(|t| comando(Comando::AlterarEndereco(t)))
                                .on_submit(comando(Comando::IrPara(a.endereco.clone())))
                                .into()
                        }),
                        p::text("ƒx")
                            .font(p::Font::SERIF)
                            .size(20)
                            .color(tema.primary),
                        Element::new(CampoTeclado {
                            manter_foco: !a
                                .editor_formula
                                .sugestoes(&a.edicao, a.config.idioma == "en-US")
                                .is_empty(),
                            conteudo: campo.into()
                        }),
                        botao("✓", Comando::Confirmar),
                    ]
                    .spacing(10)
                    .align(p::Align::Center),
                )
                .padding([6, 12]),
            );
            let sugestoes = a
                .editor_formula
                .sugestoes(&a.edicao, a.config.idioma == "en-US");
            if a.editor_formula.ativo && !sugestoes.is_empty() {
                let itens = sugestoes.into_iter().take(5).enumerate().map(|(i, s)| {
                    p::button(
                        row![
                            p::text(s.name).size(12).font(p::Font::SEMIBOLD),
                            p::text(s.signature).size(11)
                        ]
                        .spacing(12),
                    )
                    .width(Length::FILL)
                    .on_press(comando(Comando::AceitarSugestao(i)))
                    .into()
                });
                corpo = corpo.push(
                    p::container(p::column(itens).spacing(3))
                        .padding([6, 12])
                        .style(painel),
                );
            }
        }
        let grade: Element<'_, Comando> = p::program(a.grade()).into();
        corpo = corpo.push(
            p::container(grade.map(comando))
                .width(Length::FILL)
                .height(Length::FILL)
                .padding([0, 12]),
        );
        let mut folhas = p::row(Vec::<Element<'_, Msg>>::new())
            .spacing(5)
            .align(p::Align::Center);
        for indice in a.workbook.abas_visiveis() {
            let nome = a
                .workbook
                .aba(indice)
                .map(|v| v.sheet.name.as_str())
                .unwrap_or("");
            folhas = folhas.push(
                p::button(p::text(nome).size(12))
                    .padding([8, 17])
                    .on_press(comando(Comando::Aba(indice)))
                    .style(move |t, _| {
                        p::Style::default()
                            .background(if indice == a.folha {
                                t.primary.alpha(0.15)
                            } else {
                                t.surface
                            })
                            .text_color(if indice == a.folha {
                                t.primary
                            } else {
                                t.muted
                            })
                            .border(p::Border::rounded(7.).color(t.border, 1.))
                    }),
            );
        }
        corpo = corpo.push(
            row![
                p::scrollable(folhas)
                    .direction(p::Direction::Horizontal)
                    .height(45.)
                    .width(Length::FILL),
                botao("+", Comando::AdicionarAba)
            ]
            .padding([6, 12])
            .spacing(6)
            .align(p::Align::Center),
        );
        corpo = corpo.push(
            p::container(
                row![
                    p::text(a.status.as_str())
                        .size(11)
                        .width(Length::FILL)
                        .color(tema.muted),
                    p::text(a.endereco.as_str()).size(11),
                    botao("−", Comando::Zoom(a.config.zoom.saturating_sub(10))),
                    p::text(format!("{}%", a.config.zoom)).size(11),
                    botao("+", Comando::Zoom(a.config.zoom.saturating_add(10))),
                ]
                .spacing(10)
                .align(p::Align::Center),
            )
            .padding([2, 14]),
        );
        let base = p::container(corpo)
            .height(Length::FILL)
            .width(Length::FILL)
            .style(|t| p::Style::default().background(t.background));
        if a.carregando {
            return redimensionavel(
                p::stack![base, self.modal("Abrindo planilha", &a.status, vec![])].into(),
            );
        }
        if a.pendente.is_some() {
            return redimensionavel(
                p::stack![
                    base,
                    self.modal(
                        "Salvar alterações?",
                        "Esta pasta contém alterações que ainda não foram salvas.",
                        vec![
                            botao("Cancelar", Comando::Cancelar),
                            botao("Descartar", Comando::Descartar),
                            botao("Salvar", Comando::SalvarAntes)
                        ]
                    )
                ]
                .into(),
            );
        }
        if let Some(pendente) = &a.validacao_alerta {
            use validacao_dialogo::{Evento, Resposta};
            let mut acoes = vec![
                botao(
                    "Cancelar",
                    Comando::ValidacaoEv(Evento::Responder(Resposta::Cancelar)),
                ),
                botao(
                    "Corrigir",
                    Comando::ValidacaoEv(Evento::Responder(Resposta::Voltar)),
                ),
            ];
            if pendente.alerta.estilo
                != feathersheet_core::operacoes::validation::EstiloAlerta::Parar
            {
                acoes.push(botao(
                    "Aceitar",
                    Comando::ValidacaoEv(Evento::Responder(Resposta::Aceitar)),
                ));
            }
            return redimensionavel(
                p::stack![
                    base,
                    self.modal(&pendente.alerta.titulo, &pendente.alerta.texto, acoes)
                ]
                .into(),
            );
        }
        redimensionavel(base.into())
    }
}

pub(crate) fn rodar() -> Result<(), p::Error> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let captura = args.iter().position(|s| s == "--capturar");
    let inicial = args
        .iter()
        .enumerate()
        .find(|(i, s)| {
            !s.to_string_lossy().starts_with("--") && captura.map_or(true, |c| *i != c + 1)
        })
        .map(|(_, s)| PathBuf::from(s));
    let (mut app, tarefa) = Interface::nova(if captura.is_some() {
        None
    } else {
        inicial.clone()
    });
    if let Some(i) = captura {
        let destino = args
            .get(i + 1)
            .ok_or_else(|| p::Error::Window("Informe o caminho do PNG após --capturar".into()))?;
        if let Some(path) = inicial {
            let (workbook, folha) =
                Planilha::decodificar_caminho(&path, app.app.config.separador_csv())
                    .map_err(p::Error::Window)?;
            app.app.workbook = workbook;
            app.app.folha = folha;
            app.app.caminho = Some(path);
            app.app.calcular_visiveis(Duration::from_secs(1));
        }
        let png = p::capture(
            &app.view(),
            p::Size::new(1440., 900.),
            1.,
            &app.theme(),
            &mut p::Renderer::new(),
            None,
        );
        std::fs::write(destino, png).map_err(|e| p::Error::Window(e.to_string()))?;
        return Ok(());
    }
    p::run(
        p::Settings {
            title: "FeatherSheet · Pelicano".into(),
            size: p::Size::new(1440., 900.),
            min_size: Some(p::Size::new(900., 600.)),
            close_on_request: false,
            app_id: Some("feathersheet".into()),
            decorations: false,
            text_cache: true,
            ..Default::default()
        },
        || (app, tarefa),
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    fn interface() -> Interface {
        Interface {
            app: Planilha {
                status: "Pronto".into(),
                ..Default::default()
            },
            faixa: 0,
            foco: 0,
        }
    }

    #[test]
    fn editar_calcular_e_desfazer_usam_o_mesmo_motor() {
        let mut ui = interface();
        ui.update(comando(Comando::Digitar("12".into())));
        ui.update(comando(Comando::Confirmar));
        ui.update(comando(Comando::Selecionar(0, 1)));
        ui.update(comando(Comando::Digitar("=A1*2".into())));
        ui.update(comando(Comando::Confirmar));
        assert_eq!(
            ui.app.workbook.valor(0, 0, 1),
            feathersheet_core::celulas::values::Value::Number(24.)
        );
        ui.update(comando(Comando::Desfazer));
        assert_eq!(ui.app.workbook.aba(0).unwrap().sheet.get_raw(0, 1), "");
        ui.update(comando(Comando::Refazer));
        assert_eq!(
            ui.app.workbook.valor(0, 0, 1),
            feathersheet_core::celulas::values::Value::Number(24.)
        );
    }

    #[test]
    fn trocar_documento_exige_responder_ao_aviso_de_salvamento() {
        let mut ui = interface();
        ui.update(comando(Comando::Digitar("Não perder".into())));
        ui.update(comando(Comando::Confirmar));
        ui.update(comando(Comando::Novo));
        assert!(ui.app.pendente.is_some());
        ui.update(comando(Comando::Limpar));
        assert_eq!(
            ui.app.workbook.aba(0).unwrap().sheet.get_raw(0, 0),
            "Não perder"
        );
        ui.update(comando(Comando::Cancelar));
        assert!(ui.app.pendente.is_none());
        assert!(ui.app.alterado);
    }

    #[test]
    fn ponte_entrega_resultado_assincrono() {
        let tarefa = iced::Task::perform(async { "recebido".to_owned() }, Comando::Editar);
        let mut fluxo = iced_runtime::task::into_stream(tarefa).unwrap();
        use p::futures::StreamExt;
        let acao = p::futures::executor::block_on(fluxo.next()).unwrap();
        let mut ui = interface();
        ui.acao(acao);
        assert_eq!(ui.app.edicao, "recebido");
    }

    #[test]
    fn carregamento_bloqueia_digitacao_mas_recebe_o_resultado() {
        let mut ui = interface();
        ui.app.carregando = true;
        ui.update(comando(Comando::Digitar("Não descartar".into())));
        assert!(ui.app.edicao.is_empty());
        ui.acao(iced_runtime::Action::Output(Comando::Aberto(Err(
            "Falha simulada".into(),
        ))));
        assert!(!ui.app.carregando);
        assert_eq!(ui.app.status, "Falha simulada");
        ui.update(comando(Comando::Digitar("Agora pode editar".into())));
        assert_eq!(ui.app.edicao, "Agora pode editar");
    }

    #[test]
    fn campo_oculto_recebe_digitacao_e_enter_devolve_o_teclado_a_grade() {
        let mut ui = interface();
        ui.app.mostrar_fx = false;
        ui.update(comando(Comando::Digitar("1".into())));
        let mut states = p::States::default();
        let mut renderer = p::Renderer::new();
        for (key, text) in [
            (p::Key::Character("2".into()), Some("2".into())),
            (p::Key::Named(p::NamedKey::Enter), None),
        ] {
            let ev = p::Event::Key {
                key,
                text,
                modifiers: p::Modifiers::empty(),
                pressed: true,
            };
            let mut mensagens = Vec::new();
            let mut comandos = Vec::new();
            let mut clipboard = p::MemoryClipboard::default();
            {
                let view = ui.view();
                let node = p::widget::layout(
                    &view,
                    p::Size::new(1440., 900.),
                    &mut renderer,
                    &states,
                    &ui.theme(),
                );
                let mut cx = p::EventCx {
                    renderer: &mut renderer,
                    states: &mut states,
                    cursor: None,
                    messages: &mut mensagens,
                    window: &mut comandos,
                    redraw: false,
                    clipboard: &mut clipboard,
                };
                assert_eq!(
                    p::widget::dispatch(&view, &node, &ev, &mut cx),
                    p::Captured::Yes
                );
            }
            for msg in mensagens {
                ui.update(msg);
            }
        }
        assert_eq!(ui.app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "12");
        assert!(states.focus.is_none());
        assert!(!ui.app.editor_formula.ativo);
    }

    #[test]
    fn validacao_bloqueia_edicao_e_comandos_por_baixo_do_alerta() {
        use feathersheet_core::operacoes::validation::{
            NumeroOp, QuandoValidacao, Regra, Validacao,
        };
        let mut ui = interface();
        let regra = Validacao {
            ranges: vec![Rect::new(0, 0, 9, 0)],
            regra: Regra::Numero {
                op: NumeroOp::Maior,
                a: 5.,
                b: None,
                inteiro: false,
            },
            quando: QuandoValidacao::Recusar,
            mensagem: Some("Maior que cinco".into()),
            mensagens: Default::default(),
        };
        ui.app.workbook.editar(0, |s| {
            s.va = Some(serde_json::to_string(&vec![regra]).unwrap())
        });
        ui.update(comando(Comando::Digitar("3".into())));
        ui.update(comando(Comando::Confirmar));
        assert!(ui.app.validacao_alerta.is_some());
        ui.update(comando(Comando::Novo));
        assert!(ui.app.validacao_alerta.is_some());
        assert_eq!(ui.app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "");
        ui.update(comando(Comando::ValidacaoEv(
            validacao_dialogo::Evento::Responder(validacao_dialogo::Resposta::Voltar),
        )));
        ui.update(comando(Comando::Digitar("8".into())));
        ui.update(comando(Comando::Confirmar));
        assert!(ui.app.validacao_alerta.is_none());
        assert_eq!(ui.app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "8");
    }

    #[test]
    fn clique_da_grade_respeita_zoom_e_dimensoes_variaveis() {
        use p::Program;
        let mut ui = interface();
        ui.app.config.zoom = 150;
        ui.app.workbook.editar(0, |s| {
            s.col_widths.insert(0, 200);
            s.row_heights.insert(0, 40);
        });
        let grade = ui.app.grade();
        let mut estado = <Grade<'_> as Program<Comando>>::State::default();
        let mut mensagens = Vec::new();
        let b = p::Rect {
            x: 12.,
            y: 200.,
            width: 1000.,
            height: 600.,
        };
        let capturado = grade.update(
            &mut estado,
            &p::Event::MousePressed(p::MouseButton::Left),
            b,
            Some(p::Point::new(
                12. + (100. + 200. + 10.) * 1.5,
                200. + (30. + 40. + 10.) * 1.5,
            )),
            &mut |m| mensagens.push(m),
        );
        assert_eq!(capturado, p::Captured::Yes);
        assert!(matches!(mensagens.as_slice(), [Comando::Selecionar(1, 1)]));
    }

    #[test]
    fn salvar_em_background_conclui_pela_ponte() {
        use p::futures::StreamExt;
        let mut ui = interface();
        ui.update(comando(Comando::Digitar("Preservado".into())));
        ui.update(comando(Comando::Confirmar));
        let pasta =
            std::env::temp_dir().join(format!("feathersheet-pelicano-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let path = pasta.join("teste.fshz");
        let tarefa = ui.app.update(Comando::DestinoSalvar(Some(path.clone())));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut fluxo = iced_runtime::task::into_stream(tarefa).unwrap();
            while let Some(acao) = fluxo.next().await {
                ui.acao(acao);
            }
        });
        assert!(!ui.app.salvando);
        assert!(!ui.app.alterado);
        let (lido, _) = Planilha::decodificar_caminho(&path, None).unwrap();
        assert_eq!(lido.aba(0).unwrap().sheet.get_raw(0, 0), "Preservado");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(pasta).unwrap();
    }

    #[test]
    fn captura_temas_sem_janela() {
        let mut ui = interface();
        for (l, c, texto) in [
            (0, 0, "Produto"),
            (0, 1, "Quantidade"),
            (0, 2, "Valor unitário"),
            (0, 3, "Total"),
            (1, 0, "Caderno"),
            (1, 1, "24"),
            (1, 2, "19.90"),
            (1, 3, "=B2*C2"),
            (2, 0, "Caneta"),
            (2, 1, "80"),
            (2, 2, "3.50"),
            (2, 3, "=B3*C3"),
            (3, 0, "Pasta"),
            (3, 1, "16"),
            (3, 2, "12.00"),
            (3, 3, "=B4*C4"),
            (5, 0, "Total geral"),
            (5, 3, "=SUM(D2:D4)"),
        ] {
            ui.app.workbook.escrever_celula(0, l, c, texto.into());
        }
        ui.app.workbook.renomear_aba(0, "Vendas");
        ui.app.calcular_visiveis(Duration::from_secs(1));
        let mut renderer = p::Renderer::new();
        for nome in ["claro", "escuro"] {
            ui.app.config.tema_auto = false;
            ui.app.config.escuro = nome == "escuro";
            let bytes = p::capture(
                &ui.view(),
                p::Size::new(1440., 900.),
                1.,
                &ui.theme(),
                &mut renderer,
                None,
            );
            let imagem = image::load_from_memory(&bytes).expect("PNG válido");
            assert_eq!((imagem.width(), imagem.height()), (1440, 900));
            if let Some(pasta) = std::env::var_os("FEATHERSHEET_PELICANO_PREVIAS") {
                std::fs::write(
                    PathBuf::from(pasta).join(format!("feathersheet-{nome}.png")),
                    bytes,
                )
                .unwrap();
            }
        }
    }
}
