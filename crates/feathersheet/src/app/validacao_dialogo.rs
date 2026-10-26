//! Interface da janela "Validação de dados", no modelo do Excel: três abas
//! (Configurações, Mensagem de entrada, Alerta de erro) no diálogo interno
//! comum (`feather_ui::dialogo`), com Limpar tudo, Cancelar e OK no rodapé.
//! Também o alerta de erro ao digitar valor inválido (Parar, Aviso,
//! Informações) e Circular dados inválidos. Os campos e as regras ficam em
//! `validacao_ui`; aqui só se desenha e se encaminha evento.
use super::*;
use super::dialogo_interno::dlg;
use feathersheet_core::operacoes::validation::{AlertaErro, EstiloAlerta, QuandoValidacao};
use pelicano::column; use pelicano::pick_list; use pelicano::radio;
use validacao_ui::{AbaValidacao, Criterio, Formulario, Opcao};

/// Eventos novos da validação (os antigos seguem como `Msg::Validacao*`).
#[derive(Debug, Clone)]
pub enum Evento {
    Estilo(QuandoValidacao),
    EntradaTitulo(String),
    EntradaTexto(String),
    ErroTitulo(String),
    /// Resposta ao alerta de erro.
    Responder(Resposta),
    /// Dados > Validação ▾ > Circular dados inválidos.
    Circular,
    /// Dados > Validação ▾ > Limpar círculos de validação.
    LimparCirculos,
}

/// Os botões do alerta reduzidos ao efeito: Repetir/Não voltam à edição,
/// Sim/OK aceitam o valor, Cancelar descarta a digitação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resposta { Aceitar, Voltar, Cancelar }

/// Alerta aberto: o valor digitado continua no editor até a resposta.
#[derive(Debug, Clone)]
pub(super) struct AlertaPendente {
    pub alerta: AlertaErro,
}

/// Teto de círculos desenhados, como o Excel (255).
pub(super) const MAX_CIRCULOS: usize = 255;

/// Linha de campo: rótulo fixo à esquerda e controle ocupando o resto.
fn campo_rotulado<'a>(rotulo: &'static str, controle: impl Into<Element<'a, Msg>>) -> Element<'a, Msg> {
    row![
        container(text(rotulo).size(12.5)).width(Length::Fixed(118.0)),
        container(controle).width(Length::FILL),
    ].spacing(10).align(pelicano::Align::Center).into()
}

fn campo_texto<'a>(dica: &'a str, valor: &'a str, msg: impl Fn(String) -> Msg + 'a) -> Element<'a, Msg> {
    text_input(dica, valor).on_input(msg).size(13).padding([6, 8]).into()
}

/// Aba "Configurações": critério, operador e valores.
fn aba_configuracoes<'a>(f: &'a Formulario, erro: Option<&'a str>) -> Element<'a, Msg> {
    let permitir_atual = validacao_ui::PERMITIR.iter()
        .find(|o| o.valor == f.permitir()).copied();
    let permitir = pick_list(&validacao_ui::PERMITIR[..], permitir_atual,
        |o: Opcao| Msg::ValidacaoPermitir(o.valor))
        .size(12.5).width(Length::FILL);

    // Sem operador, o campo "Dados" fica como texto apagado, não como controle morto.
    let operadores = Formulario::operadores(f.criterio);
    let dados: Element<'a, Msg> = if !f.tem_operador() {
        container(text(tr!("Não se aplica a este critério")).size(12.5).color(COR_TEXTO_MUTED()))
            .padding([6, 8]).width(Length::FILL).into()
    } else {
        let atual = operadores.iter().find(|o| o.valor == f.operador).copied()
            .or(operadores.first().copied());
        pick_list(operadores, atual, |o: Opcao| Msg::ValidacaoOperador(o.valor.to_owned()))
            .size(12.5).width(Length::FILL).into()
    };

    let dica = match f.criterio {
        Criterio::Lista => tr!("Itens separados por ; ou , — ou uma origem: =A2:A9, =INDIRETO(B1)"),
        Criterio::Intervalo => tr!("Origem (ex.: B2:B20, Dados!A2:A9, um nome ou INDIRETO(B1))"),
        Criterio::Caixa => tr!("Valor marcado (padrão: VERDADEIRO)"),
        Criterio::Data => tr!("Data no formato AAAA-MM-DD"),
        Criterio::Hora => tr!("Hora no formato HH:MM"),
        Criterio::Numero => tr!("Primeiro valor numérico"),
        Criterio::Texto => tr!("Texto de comparação"),
    };
    let (rotulo_a, rotulo_b) = f.rotulos_valor();

    let mut campos = column![
        campo_rotulado(tr!("Permitir"), permitir),
        campo_rotulado(tr!("Dados"), dados),
    ].spacing(10);
    if f.usa_primeiro() {
        campos = campos.push(campo_rotulado(rotulo_a, campo_texto(dica, &f.primeiro, Msg::ValidacaoPrimeiro)));
    }
    if f.usa_segundo() {
        let dica_b = match f.criterio {
            Criterio::Caixa => tr!("Valor desmarcado (padrão: FALSO)"),
            Criterio::Hora => tr!("Hora no formato HH:MM"),
            _ => tr!("Limite superior"),
        };
        campos = campos.push(campo_rotulado(rotulo_b, campo_texto(dica_b, &f.segundo, Msg::ValidacaoSegundo)));
    }
    if f.criterio == Criterio::Intervalo {
        campos = campos.push(text(tr!("Faixa desta ou de outra aba, nome definido ou INDIRETO(célula) para a lista depender de outra célula. As opções acompanham as mudanças."))
            .size(11.5).color(COR_TEXTO_MUTED()));
    }
    column![dlg::secao(tr!("Critério de validação")), campos, erro_formulario(erro)].spacing(10).into()
}

fn erro_formulario<'a>(erro: Option<&'a str>) -> Element<'a, Msg> {
    match erro {
        Some(erro) => row![
            feather_ui::icones::icone("info-circle", 15.0),
            text(erro).size(12).color(COR_ERRO()),
        ].spacing(6).align(pelicano::Align::Center).into(),
        None => Space::new(0.0, 0).into(),
    }
}

/// Aba "Mensagem de entrada": título e texto do balão ao selecionar a célula.
fn aba_mensagem_entrada<'a>(f: &'a Formulario, erro: Option<&'a str>) -> Element<'a, Msg> {
    column![
        dlg::secao(tr!("Ao selecionar a célula, mostrar esta mensagem")),
        campo_rotulado(tr!("Título"), campo_texto(tr!("Até 32 caracteres (opcional)"), &f.entrada_titulo,
            |v| Msg::ValidacaoEv(Evento::EntradaTitulo(v)))),
        campo_rotulado(tr!("Mensagem"), campo_texto(tr!("Até 255 caracteres"), &f.entrada_texto,
            |v| Msg::ValidacaoEv(Evento::EntradaTexto(v)))),
        text(tr!("Sem título e sem mensagem, nenhum balão aparece.")).size(11.5).color(COR_TEXTO_MUTED()),
        erro_formulario(erro),
    ].spacing(10).into()
}

fn rotulo_estilo(estilo: QuandoValidacao) -> (&'static str, &'static str, &'static str) {
    match estilo {
        QuandoValidacao::Recusar => (tr!("Parar"), "hand-stop", tr!("Não aceita o valor inválido")),
        QuandoValidacao::Avisar => (tr!("Aviso"), "alert-triangle", tr!("Pergunta se deseja continuar")),
        QuandoValidacao::Informar => (tr!("Informações"), "info-circle", tr!("Só informa; o valor entra")),
    }
}

/// Aba "Alerta de erro": Parar recusa, Aviso pergunta, Informações só informa.
fn aba_alerta_erro<'a>(f: &'a Formulario, erro: Option<&'a str>) -> Element<'a, Msg> {
    let mut estilos = column(Vec::<pelicano::Element<_>>::new()).spacing(8);
    for estilo in [QuandoValidacao::Recusar, QuandoValidacao::Avisar, QuandoValidacao::Informar] {
        let (nome, icone, descricao) = rotulo_estilo(estilo);
        estilos = estilos.push(row![
            // O círculo do pelicano tem 16 px fixos (o Iced pedia 15); `size` é o do texto.
            container(radio(nome, estilo, Some(f.estilo), |e| Msg::ValidacaoEv(Evento::Estilo(e)))
                .size(12.5)).width(Length::Fixed(130.0)),
            feather_ui::icones::icone(icone, 16.0),
            text(descricao).size(12).color(COR_TEXTO_MUTED()),
        ].spacing(8).align(pelicano::Align::Center));
    }
    column![
        dlg::secao(tr!("Ao digitar dados inválidos, mostrar este alerta")),
        campo_rotulado(tr!("Estilo"), estilos),
        campo_rotulado(tr!("Título"), campo_texto(tr!("Validação de dados"), &f.erro_titulo,
            |v| Msg::ValidacaoEv(Evento::ErroTitulo(v)))),
        campo_rotulado(tr!("Mensagem de erro"), campo_texto(tr!("Sem texto, mostra o motivo padrão"), &f.mensagem,
            Msg::ValidacaoMensagem)),
        erro_formulario(erro),
    ].spacing(10).into()
}

impl App {
    pub(super) fn sobrepor_validacao<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let base = match &self.validacao_alerta {
            Some(p) => self.view_alerta_validacao(&p.alerta).sobre(base),
            None => base,
        };
        let Some(f) = &self.validacao_formulario else { return base };
        let editando = self.validacao_editando.is_some();

        let aba = |rotulo: &'static str, a: AbaValidacao| button(text(rotulo).size(12.5))
            .padding([6, 12]).style(estilo_aba_faixa(self.validacao_aba == a))
            .on_press(Msg::ValidacaoAba(a));
        let abas = row![
            aba(tr!("Configurações"), AbaValidacao::Configuracoes),
            aba(tr!("Mensagem de entrada"), AbaValidacao::MensagemEntrada),
            aba(tr!("Alerta de erro"), AbaValidacao::AlertaErro),
        ].spacing(4);

        let erro = self.validacao_erro.as_deref();
        let conteudo = match self.validacao_aba {
            AbaValidacao::Configuracoes => aba_configuracoes(f, erro),
            AbaValidacao::MensagemEntrada => aba_mensagem_entrada(f, erro),
            AbaValidacao::AlertaErro => aba_alerta_erro(f, erro),
        };

        let alvo = self.validacao_editando
            .and_then(|i| self.workbook.aba(self.folha)
                .and_then(|a| validacao_ui::carregar_regras(a.sheet.va.as_deref()).ok())
                .and_then(|regras| regras.get(i).cloned()))
            .map(|regra| regra.ranges.iter().take(3)
                .map(feathersheet_core::celulas::refs::format_ref)
                .collect::<Vec<_>>().join(", "))
            .unwrap_or_else(|| feathersheet_core::celulas::refs::format_ref(&self.area_selecionada()));

        let corpo = column![abas, container(conteudo).padding([14, 14]).width(Length::FILL)
            .style(dlg::estilo_bloco)].spacing(12);
        // Sem regra aberta não há o que limpar: o botão fica desabilitado.
        self.dialogo(if editando { tr!("Editar validação de dados") } else { tr!("Validação de dados") }, corpo)
            .icone("checks")
            .subtitulo(trf!("Intervalo de destino: {alvo}"))
            .largura(560.0)
            .ao_fechar(Msg::ValidacaoFechar)
            .rodape_esquerda(dlg::botao_secundario(tr!("Limpar tudo"), editando.then_some(Msg::ValidacaoRemover)))
            .secundario(tr!("Cancelar"), Some(Msg::ValidacaoFechar))
            .primario("OK", Some(Msg::ValidacaoSalvar))
            .sobre(base)
    }

    /// O alerta de erro do Excel: Parar (Repetir/Cancelar), Aviso
    /// (Sim/Não/Cancelar, "Continuar?") ou Informações (OK/Cancelar).
    fn view_alerta_validacao<'a>(&self, alerta: &'a AlertaErro) -> super::dialogo_interno::Dialogo<'a, Msg> {
        let r = |resposta| Some(Msg::ValidacaoEv(Evento::Responder(resposta)));
        let icone = match alerta.estilo {
            EstiloAlerta::Parar => "hand-stop",
            EstiloAlerta::Aviso => "alert-triangle",
            EstiloAlerta::Informacoes => "info-circle",
        };
        let mut texto_col = column![text(alerta.texto.clone()).size(13.5).line_height(1.4)].spacing(8);
        if alerta.estilo == EstiloAlerta::Aviso {
            texto_col = texto_col.push(text(tr!("Continuar?")).size(13.5));
        }
        let corpo = row![feather_ui::icones::icone(icone, 30.0), texto_col.width(Length::FILL)]
            .spacing(14).align(pelicano::Align::Start);
        let d = self.dialogo(alerta.titulo.clone(), corpo)
            .largura(440.0)
            .clique_fora_fecha(false)
            .ao_fechar(Msg::ValidacaoEv(Evento::Responder(Resposta::Cancelar)));
        match alerta.estilo {
            EstiloAlerta::Parar => d
                .secundario(tr!("Cancelar"), r(Resposta::Cancelar))
                .primario(tr!("Repetir"), r(Resposta::Voltar)),
            EstiloAlerta::Aviso => d
                .botao(dlg::botao_primario(tr!("Sim"), r(Resposta::Aceitar)))
                .botao(dlg::botao_secundario(tr!("Não"), r(Resposta::Voltar)))
                .botao(dlg::botao_secundario(tr!("Cancelar"), r(Resposta::Cancelar))),
            EstiloAlerta::Informacoes => d
                .botao(dlg::botao_primario("OK", r(Resposta::Aceitar)))
                .botao(dlg::botao_secundario(tr!("Cancelar"), r(Resposta::Cancelar))),
        }
    }

    /// Botão dividido da faixa Dados: o corpo abre o diálogo e a seta, o
    /// menu Validação ▾ (Circular dados inválidos, Limpar círculos).
    pub(super) fn faixa_validacao(&self) -> Element<'_, Msg> {
        row![
            button(row![
                feather_ui::icones::icone("checks", 17.),
                text(tr!("Validação de dados")).size(12),
            ].spacing(5).align(pelicano::Align::Center))
                .padding([6, 4])
                .style(estilo_botao_pequeno(self.validacao_formulario.is_some()))
                .on_press(Msg::ValidacaoAbrir),
            button(feather_ui::icones::icone("chevron-down", 13.))
                .padding([7, 3])
                .style(estilo_botao_pequeno(self.edicao_menu == Some("validacao")))
                .on_press(Msg::AlternarMenuEdicao("validacao")),
        ].spacing(1).align(pelicano::Align::Center).into()
    }

    pub(super) fn evento_validacao(&mut self, evento: Evento) -> Task<Msg> {
        let form = |app: &mut Self, f: &dyn Fn(&mut Formulario)| {
            if let Some(formulario) = &mut app.validacao_formulario { f(formulario); }
            app.validacao_erro = None;
        };
        match evento {
            Evento::Estilo(e) => form(self, &|f| f.estilo = e),
            Evento::EntradaTitulo(v) => form(self, &|f| f.entrada_titulo = v.clone()),
            Evento::EntradaTexto(v) => form(self, &|f| f.entrada_texto = v.clone()),
            Evento::ErroTitulo(v) => form(self, &|f| f.erro_titulo = v.clone()),
            Evento::Responder(resposta) => return self.responder_alerta(resposta),
            Evento::Circular => {
                self.edicao_menu = None;
                if !self.confirmar() { return Task::none(); }
                self.circular_dados_invalidos();
            }
            Evento::LimparCirculos => {
                self.edicao_menu = None;
                self.validacao_circulos = None;
                self.status = tr!("Círculos de validação removidos").into();
            }
        }
        Task::none()
    }

    /// Teclas com o alerta, a Validação ou a Série abertos: Enter é o botão
    /// padrão, Esc fecha (Cancelar). `None` = não é com eles.
    pub(super) fn tecla_validacao_serie(&mut self, tecla: &keyboard::Key) -> Option<Task<Msg>> {
        use keyboard::key::Named;
        let enter = *tecla == keyboard::Key::Named(Named::Enter);
        let esc = *tecla == keyboard::Key::Named(Named::Escape);
        if let Some(p) = &self.validacao_alerta {
            let padrao = match p.alerta.estilo {
                EstiloAlerta::Parar => Resposta::Voltar,
                _ => Resposta::Aceitar,
            };
            return Some(if enter { self.responder_alerta(padrao) }
                else if esc { self.responder_alerta(Resposta::Cancelar) }
                else { Task::none() });
        }
        if self.validacao_formulario.is_some() {
            return Some(if esc { self.update(Msg::ValidacaoFechar) } else { Task::none() });
        }
        if self.serie.is_some() {
            return Some(if enter { self.update(Msg::Serie(serie_ui::Evento::Aplicar)) }
                else if esc { self.update(Msg::Serie(serie_ui::Evento::Fechar)) }
                else { Task::none() });
        }
        None
    }

    fn responder_alerta(&mut self, resposta: Resposta) -> Task<Msg> {
        let Some(pendente) = self.validacao_alerta.take() else { return Task::none() };
        match resposta {
            // Repetir / Não: o texto digitado continua no editor para correção.
            Resposta::Voltar => {
                self.status = pendente.alerta.texto;
                Task::none()
            }
            Resposta::Cancelar => {
                self.status = tr!("Entrada cancelada").into();
                self.update(Msg::Cancelar)
            }
            Resposta::Aceitar => {
                let celula = self.editor_formula.origem.map(|(_, r, c)| cell_name(r, c));
                self.validacao_aceita = true;
                let gravou = self.confirmar();
                self.validacao_aceita = false;
                if gravou {
                    if let Some(celula) = celula {
                        self.status = trf!("{celula} atualizado (valor fora da validação aceito)");
                    }
                    return crate::campo_fx::desfocar();
                }
                Task::none()
            }
        }
    }

    /// `FEATHERSHEET_CAPTURAR_DIALOGO=validacao|validacao_entrada|validacao_alerta_aba|
    /// validacao_parar|validacao_aviso|validacao_info|validacao_balao|validacao_circulos`.
    pub(super) fn preparar_captura_validacao(&mut self, nome: &str) -> Task<Msg> {
        use feathersheet_core::operacoes::validation::{MensagensValidacao, NumeroOp, Regra, Validacao};
        let quando = match nome {
            "validacao_aviso" => QuandoValidacao::Avisar,
            "validacao_info" => QuandoValidacao::Informar,
            _ => QuandoValidacao::Recusar,
        };
        // Regra de exemplo em B2:B8: inteiro entre 1 e 10, com mensagens.
        let regra = Validacao {
            ranges: vec![Rect::new(1, 1, 7, 1)],
            regra: Regra::Numero { op: NumeroOp::Entre, a: 1.0, b: Some(10.0), inteiro: true },
            quando,
            mensagem: Some(tr!("A nota deve ser um número inteiro de 1 a 10.").into()),
            mensagens: MensagensValidacao {
                entrada_titulo: Some(tr!("Nota da avaliação").into()),
                entrada_texto: Some(tr!("Digite um número inteiro de 1 a 10.").into()),
                erro_titulo: Some(tr!("Nota inválida").into()),
            },
        };
        self.workbook.editar(self.folha, |s| s.va = Some(serde_json::to_string(&vec![regra]).unwrap_or_default()));
        for (r, v) in [(0, tr!("Nota")), (1, "7"), (2, "12"), (3, "5"), (4, "0"), (5, "9"), (6, "8,5")] {
            self.workbook.escrever_celula(self.folha, r, 1, v.into());
        }
        self.trocar_selecao(2, 1);
        self.fim_selecao = (2, 1);
        match nome {
            "validacao_balao" => {}
            // Dados > Validação ▾ aberto, com a seta logo abaixo do botão.
            "validacao_menu" => {
                self.faixa_ativa = AbaFaixa::Dados;
                let _ = self.update(Msg::AlternarMenuEdicao("validacao"));
                self.edicao_popup_x = 560.0;
                self.edicao_popup_y = 150.0;
            }
            "validacao_circulos" => { let _ = self.evento_validacao(Evento::Circular); }
            "validacao_parar" | "validacao_aviso" | "validacao_info" => {
                self.trocar_selecao(7, 1);
                self.fim_selecao = (7, 1);
                let _ = self.update(Msg::Digitar("15".into()));
                let _ = self.confirmar();
            }
            _ => {
                let _ = self.update(Msg::ValidacaoAbrir);
                let aba = match nome {
                    "validacao_entrada" => AbaValidacao::MensagemEntrada,
                    "validacao_alerta_aba" => AbaValidacao::AlertaErro,
                    _ => AbaValidacao::Configuracoes,
                };
                let _ = self.update(Msg::ValidacaoAba(aba));
            }
        }
        Task::none()
    }

    /// Recalcula os círculos da aba ativa a partir do motor.
    pub(super) fn circular_dados_invalidos(&mut self) {
        let regras = match validacao_ui::carregar_regras(
            self.workbook.aba(self.folha).and_then(|a| a.sheet.va.as_deref()))
        {
            Ok(regras) => regras,
            Err(erro) => { self.status = erro; return; }
        };
        let mut celulas = self.workbook.circular_dados_invalidos(self.folha, &regras);
        let total = celulas.len();
        celulas.truncate(MAX_CIRCULOS);
        self.status = match total {
            0 => tr!("Nenhum dado inválido nesta aba").into(),
            1 => tr!("1 célula com dado inválido circulada").into(),
            n if n > MAX_CIRCULOS => trf!("{n} células inválidas; circuladas as primeiras {MAX_CIRCULOS}"),
            n => trf!("{n} células com dado inválido circuladas"),
        };
        self.validacao_circulos = Some((self.folha, celulas));
    }

    /// Depois de editar: a célula corrigida perde o círculo (o Excel faz o mesmo).
    pub(super) fn atualizar_circulos(&mut self) {
        if self.validacao_circulos.as_ref().is_some_and(|(aba, _)| *aba == self.folha) {
            let status = std::mem::take(&mut self.status);
            self.circular_dados_invalidos();
            self.status = status;
        }
    }

    /// Círculos da aba ativa, para a grade.
    pub(super) fn circulos_visiveis(&self) -> &[(u32, u32)] {
        match &self.validacao_circulos {
            Some((aba, celulas)) if *aba == self.folha => celulas,
            _ => &[],
        }
    }

    pub(super) fn validar_entrada(&self, folha: usize, linha: u32, coluna: u32, texto: &str)
        -> Result<Option<String>, String>
    {
        let json = self.workbook.aba(folha).and_then(|a| a.sheet.va.as_deref());
        let regras = validacao_ui::carregar_regras(json)?;
        validacao_ui::validar_celula(&self.workbook, folha, linha, coluna, texto, &regras)
    }

    /// O alerta que a digitação provoca (Parar, Aviso ou Informações).
    pub(super) fn alerta_validacao(&self, folha: usize, linha: u32, coluna: u32, texto: &str)
        -> Result<Option<AlertaErro>, String>
    {
        let json = self.workbook.aba(folha).and_then(|a| a.sheet.va.as_deref());
        let regras = validacao_ui::carregar_regras(json)?;
        Ok(validacao_ui::alerta_da_celula(&self.workbook, folha, linha, coluna, texto, &regras))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::operacoes::validation::{NumeroOp, Regra, Validacao};

    fn com_regra(quando: QuandoValidacao) -> App {
        let mut app = App::default();
        let regra = Validacao {
            ranges: vec![Rect::new(0, 0, 9, 0)],
            regra: Regra::Numero { op: NumeroOp::Maior, a: 5.0, b: None, inteiro: false },
            quando, mensagem: Some("Maior que cinco".into()),
            mensagens: feathersheet_core::operacoes::validation::MensagensValidacao {
                entrada_titulo: Some("Quantidade".into()), entrada_texto: Some("Digite mais que 5".into()),
                erro_titulo: Some("Valor baixo".into()) },
        };
        let json = serde_json::to_string(&vec![regra]).unwrap();
        app.workbook.editar(0, |s| s.va = Some(json));
        app
    }

    fn digitar(app: &mut App, texto: &str) -> bool {
        let _ = app.update(Msg::Digitar(texto.into()));
        app.confirmar()
    }

    fn raw(app: &App, r: u32) -> String { app.workbook.aba(0).unwrap().sheet.get_raw(r, 0) }

    #[test]
    fn parar_recusa_e_repetir_volta_ao_editor() {
        let mut app = com_regra(QuandoValidacao::Recusar);
        assert!(!digitar(&mut app, "3"));
        let alerta = &app.validacao_alerta.as_ref().unwrap().alerta;
        assert_eq!((alerta.estilo, alerta.titulo.as_str(), alerta.texto.as_str()),
            (EstiloAlerta::Parar, "Valor baixo", "Maior que cinco"));
        let _: Element<'_, Msg> = app.sobrepor_validacao(Space::new(0.0, 0).into());
        let _ = app.update(Msg::ValidacaoEv(Evento::Responder(Resposta::Voltar)));
        assert!(app.validacao_alerta.is_none() && app.editor_formula.ativo);
        assert_eq!(raw(&app, 0), "");
        // Cancelar descarta a digitação.
        assert!(!app.confirmar());
        let _ = app.update(Msg::ValidacaoEv(Evento::Responder(Resposta::Cancelar)));
        assert!(app.validacao_alerta.is_none() && !app.editor_formula.ativo);
        assert_eq!(raw(&app, 0), "");
    }

    #[test]
    fn aviso_sim_aceita_e_um_ctrl_z_desfaz() {
        let mut app = com_regra(QuandoValidacao::Avisar);
        assert!(!digitar(&mut app, "3"));
        assert_eq!(app.validacao_alerta.as_ref().unwrap().alerta.estilo, EstiloAlerta::Aviso);
        let _ = app.update(Msg::ValidacaoEv(Evento::Responder(Resposta::Aceitar)));
        assert!(app.validacao_alerta.is_none() && !app.editor_formula.ativo);
        assert_eq!(raw(&app, 0), "3");
        assert!(app.status.contains("aceito"), "{}", app.status);
        // Valor válido não mostra alerta.
        let _ = app.update(Msg::Mover(1, 0));
        assert!(digitar(&mut app, "9"));
        assert!(app.validacao_alerta.is_none());
        let _ = app.update(Msg::Desfazer);
        assert_eq!(raw(&app, 1), "");
    }

    #[test]
    fn informacoes_ok_aceita() {
        let mut app = com_regra(QuandoValidacao::Informar);
        assert!(!digitar(&mut app, "1"));
        assert_eq!(app.validacao_alerta.as_ref().unwrap().alerta.estilo, EstiloAlerta::Informacoes);
        let _ = app.update(Msg::ValidacaoEv(Evento::Responder(Resposta::Aceitar)));
        assert_eq!(raw(&app, 0), "1");
    }

    #[test]
    fn dialogo_salva_mensagens_e_estilo() {
        let mut app = App::default();
        let _ = app.update(Msg::ValidacaoAbrir);
        let _ = app.update(Msg::ValidacaoPermitir("hora"));
        let _ = app.update(Msg::ValidacaoPrimeiro("08:00".into()));
        let _ = app.update(Msg::ValidacaoSegundo("18:00".into()));
        let _ = app.update(Msg::ValidacaoAba(AbaValidacao::MensagemEntrada));
        let _ = app.update(Msg::ValidacaoEv(Evento::EntradaTitulo("Horário".into())));
        let _ = app.update(Msg::ValidacaoEv(Evento::EntradaTexto("Expediente".into())));
        let _ = app.update(Msg::ValidacaoAba(AbaValidacao::AlertaErro));
        let _ = app.update(Msg::ValidacaoEv(Evento::Estilo(QuandoValidacao::Avisar)));
        let _: Element<'_, Msg> = app.sobrepor_validacao(Space::new(0.0, 0).into());
        let _ = app.update(Msg::ValidacaoSalvar);
        assert!(app.validacao_formulario.is_none(), "{:?}", app.validacao_erro);
        let regras = validacao_ui::carregar_regras(app.workbook.aba(0).unwrap().sheet.va.as_deref()).unwrap();
        assert_eq!(regras[0].quando, QuandoValidacao::Avisar);
        assert_eq!(regras[0].mensagem_de_entrada(), Some(("Horário".into(), "Expediente".into())));
        assert!(matches!(regras[0].regra, Regra::Hora { .. }));
        // Reabrir mostra o que foi salvo.
        let _ = app.update(Msg::ValidacaoAbrir);
        let f = app.validacao_formulario.as_ref().unwrap();
        assert_eq!((f.entrada_titulo.as_str(), f.estilo), ("Horário", QuandoValidacao::Avisar));
    }

    #[test]
    fn circular_e_limpar_circulos() {
        let mut app = com_regra(QuandoValidacao::Recusar);
        for (r, v) in [(0, "3"), (1, "9"), (2, "1")] {
            app.workbook.escrever_celula(0, r, 0, v.into());
        }
        let _ = app.update(Msg::ValidacaoEv(Evento::Circular));
        assert_eq!(app.circulos_visiveis(), &[(0, 0), (2, 0)]);
        // Corrigir uma célula tira o círculo dela.
        let _ = app.update(Msg::Digitar("8".into()));
        assert!(app.confirmar());
        assert_eq!(app.circulos_visiveis(), &[(2, 0)]);
        let _ = app.update(Msg::ValidacaoEv(Evento::LimparCirculos));
        assert!(app.circulos_visiveis().is_empty());
    }

    #[test]
    fn menu_validacao_tem_circular_e_limpar() {
        let mut app = App::default();
        let _ = app.update(Msg::AlternarMenuEdicao("validacao"));
        assert_eq!(app.edicao_menu, Some("validacao"));
        let _ = app.update(Msg::EdicaoMenuAcao(Box::new(Msg::ValidacaoEv(Evento::Circular))));
        assert!(app.edicao_menu.is_none());
        assert_eq!(app.status, "Nenhum dado inválido nesta aba");
    }
}
