//! Caneta de borda da Página Inicial (Bordas ▾ > Desenhar bordas), como no
//! Excel: "Desenhar borda" (contorno do arraste), "Desenhar grade de borda"
//! e "Apagar borda", com cor e estilo da linha. O arraste na grade é o mesmo
//! do Pincel de formatação: a grade avisa ao soltar o mouse
//! (`Msg::AplicarPincel`) e a área selecionada recebe o traço, um Ctrl+Z por
//! traço. Esc ou o mesmo item do menu saem do modo. A regra fica no motor
//! (`feathersheet_core::operacoes::desenho_borda`).
use super::*;
use feathersheet_core::operacoes::desenho_borda::{desenhar_borda, CanetaBorda, ModoBorda};

/// Modo atual da caneta e a caneta escolhida (vale até fechar o app).
#[derive(Debug, Clone, Default)]
pub(crate) struct Estado {
    pub(crate) modo: Option<ModoBorda>,
    pub(crate) caneta: CanetaBorda,
}

#[derive(Debug, Clone)]
pub enum Evento {
    /// Entra no modo (ou sai, se já estava nele).
    Modo(ModoBorda),
    /// Cor da linha (`None` = automática). Como no Excel, liga o desenho.
    Cor(Option<&'static str>),
    /// Estilo da linha (Fina, Média, Grossa, Tracejada). Também liga o desenho.
    Estilo(&'static str),
    Sair,
}

/// Cores da caneta, as mesmas da lista rápida de cor do texto/fundo.
pub(super) const CORES: &[(&str, Option<&str>)] = &[
    ("Automática", None),
    ("Preto", Some("#1f1f1f")),
    ("Cinza", Some("#808080")),
    ("Vermelho", Some("#d93025")),
    ("Laranja", Some("#e8710a")),
    ("Verde", Some("#188038")),
    ("Azul", Some("#0b57d0")),
];

pub(super) fn nome_modo(modo: ModoBorda) -> &'static str {
    match modo {
        ModoBorda::Contorno => tr!("Desenhar borda"),
        ModoBorda::Grade => tr!("Desenhar grade de borda"),
        ModoBorda::Apagar => tr!("Apagar borda"),
    }
}

impl App {
    pub(super) fn evento_desenho_borda(&mut self, evento: Evento) -> Task<Msg> {
        self.bordas_menu_aberto = false;
        let modo = match evento {
            Evento::Sair => {
                self.borda_desenho.modo = None;
                self.status = tr!("Desenho de bordas encerrado").into();
                return Task::none();
            }
            Evento::Modo(m) if self.borda_desenho.modo == Some(m) => {
                return self.evento_desenho_borda(Evento::Sair);
            }
            Evento::Modo(m) => m,
            Evento::Cor(cor) => {
                self.borda_desenho.caneta.cor = cor.map(str::to_owned);
                self.borda_desenho.modo.filter(|m| *m != ModoBorda::Apagar).unwrap_or(ModoBorda::Contorno)
            }
            Evento::Estilo(estilo) => {
                self.borda_desenho.caneta.estilo = (estilo != "Fina").then(|| estilo.to_owned());
                self.borda_desenho.modo.filter(|m| *m != ModoBorda::Apagar).unwrap_or(ModoBorda::Contorno)
            }
        };
        if !self.confirmar() { return Task::none(); }
        // A caneta de borda e o pincel usam o mesmo arraste: um de cada vez.
        self.pincel_formatacao = None;
        self.borda_desenho.modo = Some(modo);
        self.status = trf!("{}: arraste sobre as células. Esc sai.", nome_modo(modo));
        Task::none()
    }

    /// Fim do arraste na grade com a caneta ligada: aplica o traço na área
    /// selecionada. Devolve `false` se a caneta não está ligada.
    pub(super) fn aplicar_desenho_borda(&mut self) -> bool {
        let Some(modo) = self.borda_desenho.modo else { return false };
        let area = self.area_selecionada();
        let caneta = self.borda_desenho.caneta.clone();
        let mut resultado = Ok(false);
        self.workbook.editar(self.folha, |sheet| {
            resultado = desenhar_borda(sheet, &area, modo, &caneta);
        });
        match resultado {
            Ok(true) => {
                self.alterado = true;
                self.status = trf!("{}: traço aplicado (Ctrl+Z desfaz). Esc sai.", nome_modo(modo));
            }
            Ok(false) => self.status = trf!("{}: nada mudou. Esc sai.", nome_modo(modo)),
            Err(erro) => self.status = erro,
        }
        true
    }

    /// `FEATHERSHEET_CAPTURAR_INICIO=duplo,bordas,…`: prepara a foto da
    /// Página Inicial. Itens: `duplo`/`simples` (sublinhado na seleção),
    /// `recuo` (+2 níveis), `grade`/`contorno`/`apagar` (traço da caneta na
    /// seleção, cor vermelha e linha grossa), `modo` (deixa a caneta ligada),
    /// `menu:bordas` ou `menu:<tipo>` (abre o menu: limpar, classificar,
    /// sublinhado…), `celula:B3` ou `celula:B3:D6` (muda a seleção).
    pub(super) fn preparar_captura_inicio(&mut self, spec: &str) {
        use feathersheet_core::celulas::refs::parse_cell_ref;
        for item in spec.split(',').map(str::trim) {
            match item {
                "duplo" => self.alternar_estilo_menu(feathersheet_core::operacoes::sublinhado::CHAVE_DUPLO),
                "simples" => self.alternar_estilo_menu("underline"),
                "recuo" => { let _ = self.update(Msg::AlterarRecuo(2)); }
                "grade" | "contorno" | "apagar" => {
                    self.borda_desenho.caneta = CanetaBorda {
                        cor: Some("#d93025".into()), estilo: Some("Grossa".into()),
                    };
                    self.borda_desenho.modo = Some(match item {
                        "grade" => ModoBorda::Grade,
                        "apagar" => ModoBorda::Apagar,
                        _ => ModoBorda::Contorno,
                    });
                    self.aplicar_desenho_borda();
                    self.borda_desenho.modo = None;
                }
                "modo" => { let _ = self.evento_desenho_borda(Evento::Modo(ModoBorda::Contorno)); }
                // "menu:orientacao@x,y": o menu como se o botão fosse clicado em (x, y).
                _ if item.starts_with("menu:orientacao") => {
                    let (x, y) = item.split_once('@').and_then(|(_, p)| p.split_once(','))
                        .and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))).unwrap_or((600.0, 120.0));
                    self.ponteiro_x = x;
                    self.ponteiro_y = y;
                    let _ = self.update(Msg::AlternarMenuOrientacao);
                }
                "menu:bordas" => {
                    self.bordas_popup_x = 300.0;
                    self.bordas_popup_y = 150.0;
                    self.bordas_menu_aberto = true;
                }
                _ => {
                    if let Some(tipo) = item.strip_prefix("menu:") {
                        let tipo: &'static str = match tipo {
                            "limpar" => "limpar", "classificar" => "classificar",
                            "sublinhado" => "sublinhado", "formatar" => "formatar",
                            _ => continue,
                        };
                        let _ = self.update(Msg::AlternarMenuEdicao(tipo));
                        self.edicao_popup_x = if tipo == "sublinhado" { 250.0 } else { 1000.0 };
                        self.edicao_popup_y = 150.0;
                    } else if let Some(alvo) = item.strip_prefix("celula:") {
                        let (inicio, fim) = alvo.split_once(':').unwrap_or((alvo, alvo));
                        if let (Some((r, c)), Some(f)) = (parse_cell_ref(inicio), parse_cell_ref(fim)) {
                            self.trocar_selecao(r, c);
                            self.fim_selecao = f;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn lados(app: &App, l: u32, c: u32) -> (bool, bool, bool, bool) {
        let b = app.workbook.aba(app.folha).unwrap().sheet.estilo_resolvido(l, c)
            .and_then(|s| s.borders).unwrap_or_default();
        (b.top, b.right, b.bottom, b.left)
    }

    #[test]
    fn caneta_desenha_a_cada_soltar_e_esc_sai() {
        let mut app = App::default();
        app.update(Msg::AlternarMenuBordas);
        app.update(Msg::BordaDesenho(Evento::Cor(Some("#0b57d0"))));
        assert!(!app.bordas_menu_aberto, "escolher a cor fecha o menu");
        assert_eq!(app.borda_desenho.modo, Some(ModoBorda::Contorno), "a cor liga a caneta, como no Excel");
        // Arraste de B2 a C3: a grade manda Selecionar, Estender e, ao soltar, AplicarPincel.
        app.update(Msg::Selecionar(1, 1));
        app.update(Msg::Estender(2, 2));
        app.update(Msg::AplicarPincel);
        assert_eq!(lados(&app, 1, 1), (true, false, false, true));
        assert_eq!(lados(&app, 2, 2), (false, true, true, false));
        let cor = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(1, 1).unwrap().borders.unwrap().color;
        assert_eq!(cor.as_deref(), Some("#0b57d0"));

        // Grade, depois Apagar: cada traço é um Ctrl+Z.
        app.update(Msg::BordaDesenho(Evento::Modo(ModoBorda::Grade)));
        app.update(Msg::Selecionar(5, 0));
        app.update(Msg::AplicarPincel);
        assert_eq!(lados(&app, 5, 0), (true, true, true, true));
        app.update(Msg::BordaDesenho(Evento::Modo(ModoBorda::Apagar)));
        app.update(Msg::Selecionar(1, 1));
        app.update(Msg::Estender(2, 2));
        app.update(Msg::AplicarPincel);
        assert_eq!(lados(&app, 1, 1), (false, false, false, false));
        app.update(Msg::Desfazer);
        assert_eq!(lados(&app, 1, 1), (true, false, false, true));
        app.update(Msg::Desfazer);
        assert_eq!(lados(&app, 5, 0), (false, false, false, false));

        // Esc sai do modo; soltar o mouse depois não desenha nada.
        let id = window::novo_id();
        app.janelas.principal = Some(id);
        app.update(Msg::Teclado(id,
            keyboard::Key::Named(keyboard::key::Named::Escape),
            keyboard::Modifiers::default(),
        ));
        assert!(app.borda_desenho.modo.is_none());
        app.update(Msg::Selecionar(8, 3));
        app.update(Msg::AplicarPincel);
        assert_eq!(lados(&app, 8, 3), (false, false, false, false));
    }

    #[test]
    fn menu_de_bordas_mostra_a_caneta() {
        let mut app = App::default();
        app.update(Msg::AlternarMenuBordas);
        let _: Element<'_, Msg> = app.sobrepor_menu_bordas(Space::new(Length::FILL, Length::FILL).into());
        app.update(Msg::BordaDesenho(Evento::Estilo("Tracejada")));
        assert_eq!(app.borda_desenho.caneta.estilo.as_deref(), Some("Tracejada"));
        app.update(Msg::BordaDesenho(Evento::Modo(ModoBorda::Contorno)));
        assert!(app.borda_desenho.modo.is_none(), "o mesmo item desliga");
    }
}
