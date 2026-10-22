//! "Primeiros passos": assistente de cinco passos que abre sozinho na primeira
//! vez (e depois em Ajuda ▸ Primeiros passos). Segue a F1 (GUIA-JANELAS.md):
//! lateral com os passos, cabeçalho com a marca, Anterior/Próximo no pé.
//! Os controles mudam as mesmas preferências das Configurações, pelas mesmas
//! mensagens; aqui só moram os "Seus dados" (autor e organização).
use super::*;
use super::comentarios_ui::autor_do_sistema;
use super::configuracoes_ui::{amostras_acento, linha, linhas_salvamento, lista_idioma, opcoes_tema};
use feather_ui::consulta::{self, Ilustracao};

/// Os passos, na ordem: título e ícone da lateral.
pub const PASSOS: [(&str, &str); 5] = [
    ("Boas-vindas", "rocket"),
    ("Seus dados", "user"),
    ("Salvamento", "device-floppy"),
    ("Aparência", "palette"),
    ("Pronto", "circle-check"),
];

#[derive(Debug, Clone)]
pub enum Evento {
    Passo(usize),
    Anterior,
    Proximo,
    Nome(String),
    Iniciais(String),
    Organizacao(String),
    /// Começar (último passo) ou Pular: fecha a janela.
    Concluir,
}

/// Campo do formulário dos "Seus dados": rótulo em cima, o campo logo
/// abaixo (largura própria) e a dica curta embaixo, tudo alinhado à esquerda.
fn campo_formulario<'a>(titulo: &str, dica: &str, controle: Element<'a, Msg>) -> Element<'a, Msg> {
    pelicano::column![
        text(titulo.to_owned()).size(13.5).color(COR_TEXTO()).font(pelicano::Font { weight: 600, ..pelicano::Font::DEFAULT }),
        controle,
        text(dica.to_owned()).size(11.5).color(COR_TEXTO_MUTED()),
    ].spacing(6).into()
}

/// Iniciais de um nome: primeira letra da primeira e da última palavra.
pub(crate) fn iniciais_de(nome: &str) -> String {
    let palavras: Vec<&str> = nome.split_whitespace().collect();
    let letra = |p: &&str| p.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    match palavras.as_slice() {
        [] => String::new(),
        [so] => letra(so),
        [primeira, .., ultima] => letra(primeira) + &letra(ultima),
    }
}

impl App {
    /// Quem assina comentários e arquivos: o nome dos "Seus dados" ou, sem
    /// ele, o nome do sistema.
    pub(super) fn autor(&self) -> String {
        let nome = self.config.autor_nome.trim();
        if nome.is_empty() { autor_do_sistema() } else { nome.to_owned() }
    }

    /// Iniciais do autor de um comentário: as escolhidas pela pessoa quando o
    /// autor é ela, senão as do nome.
    pub(super) fn iniciais_do_autor(&self, autor: &str) -> String {
        let proprias = self.config.autor_iniciais.trim();
        if !proprias.is_empty() && autor == self.autor() { proprias.to_uppercase() } else { iniciais_de(autor) }
    }

    /// Antes de gravar (.fshz ou .xlsx): o arquivo sem autor ganha o da
    /// pessoa; "modificado por" e organização vazia são sempre atualizados.
    pub(super) fn carimbar_propriedades(&mut self) {
        let autor = self.autor();
        let organizacao = self.config.autor_organizacao.trim().to_owned();
        let p = &mut self.workbook.propriedades;
        if p.autor.is_empty() { p.autor = autor.clone(); }
        p.modificado_por = autor;
        if p.organizacao.is_empty() { p.organizacao = organizacao; }
    }

    /// Salvamento automático ligado, mas o documento alterado não é um .fshz
    /// salvo (xlsx, csv, parquet, planilha nova): avisa no rodapé, uma vez
    /// por documento, que só a recuperação automática o protege.
    pub(super) fn avisar_fora_do_salvamento_automatico(&mut self) {
        // Os prints do modo de captura não ganham a faixa sozinha.
        let capturando = !cfg!(test) && std::env::var_os("FEATHERSHEET_CAPTURAR").is_some();
        if !self.alterado || self.carregando || capturando { return; }
        let nome = self.nome_documento();
        if self.aviso_salvamento_de.as_deref() == Some(nome.as_str()) { return; }
        let celulas = (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i))
            .map(|aba| aba.sheet.len() as u64).fold(0u64, u64::saturating_add);
        if feathersheet_io::backup_policy::grande_demais_para_copia_automatica(celulas)
            && (self.config.auto_salvar || self.config.recuperacao_automatica)
        {
            self.monitor.registrar(Nivel::Aviso, tr!("Salvamento"), &trf!(
                "Pasta com {} milhões de células: grande demais para cópia automática (recuperação e salvamento automático desligados nela; cada cópia travaria a janela e dobraria a memória). Salve com Ctrl+S.",
                celulas / 1_000_000));
            self.faixa_salvamento = Some((trf!(
                "Esta pasta tem {} milhões de células: grande demais para cópia automática. Nem a recuperação nem o salvamento automático a gravam; salve com Ctrl+S.",
                celulas / 1_000_000), true));
            self.aviso_salvamento_de = Some(nome);
            return;
        }
        let coberto = self.caminho.as_ref()
            .is_some_and(|p| p.extension().and_then(|e| e.to_str()) == Some("fshz"));
        if !self.config.auto_salvar || coberto { return; }
        // Modelo ou cópia recuperada também têm origem .fshz sem caminho:
        // para a pessoa, é uma planilha que ainda não foi salva.
        let tipo = self.origem.as_ref().and_then(|p| p.extension()).and_then(|e| e.to_str())
            .map(str::to_lowercase).filter(|e| e != "fshz")
            .map_or("uma planilha ainda não salva".to_owned(), |e| format!("arquivos .{e}"));
        let mensagem = trf!("O salvamento automático não vale para {tipo}. Até salvar como .fshz, só a recuperação automática protege esta planilha.");
        self.monitor.registrar(Nivel::Info, tr!("Salvamento"), &mensagem);
        self.faixa_salvamento = Some((mensagem, false));
        self.aviso_salvamento_de = Some(nome);
    }

    /// Faixa fina acima da grade com o aviso de salvamento e a ação que
    /// resolve. Some sozinha quando o documento passa a ser coberto.
    pub(super) fn view_faixa_salvamento(&self) -> Element<'_, Msg> {
        let coberto = self.caminho.as_ref()
            .is_some_and(|p| p.extension().and_then(|e| e.to_str()) == Some("fshz"));
        let Some((mensagem, grande)) = self.faixa_salvamento.as_ref()
            .filter(|(_, grande)| *grande || !coberto)
            .filter(|_| self.aviso_salvamento_de.as_deref() == Some(self.nome_documento().as_str()))
        else { return Space::new(0.0, 0.0).into() };
        let acao = if *grande {
            button(text(tr!("Salvar (Ctrl+S)")).size(12)).padding([5, 12]).style(estilo_botao_primario).on_press(Msg::Salvar)
        } else {
            button(text(tr!("Salvar como .fshz…")).size(12)).padding([5, 12]).style(estilo_botao_primario).on_press(Msg::SalvarComo)
        };
        container(row![
            feather_ui::icones::icone_traco(if *grande { "alert-triangle" } else { "info-circle" }, 16.),
            text(mensagem.clone()).size(12.5).width(Length::FILL),
            acao,
            button(feather_ui::icones::icone_traco("x", 14.)).padding([5, 7])
                .style(estilo_botao_pequeno(false)).on_press(Msg::FaixaSalvamentoFechar),
        ].spacing(10).align(pelicano::Align::Center))
            .padding([6, 12]).width(Length::FILL)
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_AZUL_FUNDO())),
                border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1., ..Default::default() },
                ..Default::default()
            }).into()
    }

    /// Na abertura: a primeira vez mostra os Primeiros passos; depois de uma
    /// atualização, O que há de novo. Nunca nos testes nem na captura de tela.
    pub(super) fn janela_de_boas_vindas(&mut self) -> Option<Auxiliar> {
        if cfg!(test) || std::env::var_os("FEATHERSHEET_CAPTURAR").is_some() { return None; }
        self.boas_vindas_pendente()
    }

    /// A decisão de [`App::janela_de_boas_vindas`], sem as travas (testável).
    pub(super) fn boas_vindas_pendente(&mut self) -> Option<Auxiliar> {
        let atual = super::atualizacao::versao_atual();
        // Instalação nova: sem preferências de antes. Quem já usava uma
        // versão anterior (até a 0.0.2 não havia `versao_vista`) está
        // atualizando e vê as novidades; os Primeiros passos ficam no menu.
        let instalacao_nova = !self.config.existia && !self.config.primeiros_passos_vistos;
        let janela = if instalacao_nova {
            self.config.primeiros_passos_vistos = true;
            Some(Auxiliar::PrimeirosPassos)
        } else if self.config.versao_vista != atual {
            self.config.primeiros_passos_vistos = true;
            self.novidades_versao = 0;
            Some(Auxiliar::Novidades)
        } else {
            None
        };
        if janela.is_some() {
            // Fechar pelo X também conta como visto: não volta a abrir sozinha.
            self.config.versao_vista = atual.to_owned();
            self.config.gravar();
        }
        janela
    }

    pub(super) fn evento_primeiros_passos(&mut self, evento: Evento) -> Task<Msg> {
        let ultimo = PASSOS.len() - 1;
        match evento {
            Evento::Passo(i) => self.primeiros_passos_passo = i.min(ultimo),
            Evento::Anterior => self.primeiros_passos_passo = self.primeiros_passos_passo.saturating_sub(1),
            Evento::Proximo if self.primeiros_passos_passo >= ultimo => return self.update(Msg::PrimeirosPassos(Evento::Concluir)),
            Evento::Proximo => self.primeiros_passos_passo += 1,
            Evento::Nome(t) => { self.config.autor_nome = t.chars().take(80).collect(); self.config.gravar(); }
            Evento::Iniciais(t) => {
                self.config.autor_iniciais = t.chars().filter(|c| !c.is_whitespace()).take(4).collect::<String>().to_uppercase();
                self.config.gravar();
            }
            Evento::Organizacao(t) => { self.config.autor_organizacao = t.chars().take(80).collect(); self.config.gravar(); }
            Evento::Concluir => {
                self.primeiros_passos_passo = 0;
                if let Some(id) = self.janelas.id(Auxiliar::PrimeirosPassos) {
                    return self.update(Msg::Fechar(id));
                }
            }
        }
        Task::none()
    }

    pub(super) fn view_primeiros_passos(&self) -> Element<'_, Msg> {
        let passo = self.primeiros_passos_passo.min(PASSOS.len() - 1);
        let mut nav = pelicano::column![consulta::rotulo_grupo(tr!("Passos"))].spacing(2);
        for (i, (titulo, icone)) in PASSOS.iter().enumerate() {
            nav = nav.push(consulta::item_nav(format!("{}. {}", i + 1, tr!(titulo)), icone, i == passo,
                Some(Msg::PrimeirosPassos(Evento::Passo(i)))));
        }
        let pular = button(text(tr!("Pular e configurar depois")).size(12))
            .padding([8, 12]).width(Length::FILL).style(consulta::estilo_cartao)
            .on_press(Msg::PrimeirosPassos(Evento::Concluir));
        let ultimo = passo + 1 == PASSOS.len();
        let rodape = consulta::paginacao(
            (tr!("Anterior"), (passo > 0).then_some(Msg::PrimeirosPassos(Evento::Anterior))),
            (if ultimo { tr!("Começar") } else { tr!("Próximo") }, Some(Msg::PrimeirosPassos(Evento::Proximo))),
        );
        consulta::janela(
            consulta::lateral(nav, (!ultimo).then(|| pular.into())),
            self.conteudo_passo(passo),
            Some(rodape),
        )
    }

    fn conteudo_passo(&self, passo: usize) -> Element<'_, Msg> {
        use pelicano::column;
        // Função (não closure) para o campo poder emprestar `valor` pelo tempo da tela.
        fn campo<'a>(dica: String, valor: &'a str, ao_digitar: fn(String) -> Evento, largura: f32) -> Element<'a, Msg> {
            pelicano::text_input(dica, valor)
                .on_input(move |t| Msg::PrimeirosPassos(ao_digitar(t)))
                .padding([9, 12]).size(13.5).width(Length::Fixed(largura))
                .style(|tema, status| {
                    let mut e = pelicano::input_default(tema, status);
                    e.background = Some(pelicano::Background::Color(COR_PAINEL()));
                    e.border = pelicano::Border {
                        color: match status {
                            pelicano::Status::Focused => COR_AZUL_ACENTO(),
                            pelicano::Status::Hovered => COR_TEXTO(),
                            _ => COR_TEXTO_MUTED(),
                        },
                        width: if matches!(status, pelicano::Status::Focused) { 2. } else { 1. },
                        radius: [8.0; 4],
                    };
                    e
                }).into()
        }
        match passo {
            0 => column![
                consulta::cabecalho(tr!("Boas-vindas ao FeatherSheet"),
                    tr!("Planilhas rápidas no seu computador, sem conta e sem telemetria. Em quatro passos curtos o app fica do seu jeito; tudo pode ser mudado depois em Configurações."),
                    Some(consulta::ilustracao(Ilustracao::Planilha))),
                consulta::negrito(tr!("O que você encontra aqui"), 18.),
                consulta::grade(vec![
                    consulta::cartao("folder-open", tr!("Arquivos grandes"),
                        tr!("Abre .xlsx, .csv e .parquet com milhões de linhas sem travar a janela."), None),
                    consulta::cartao("lock", tr!("Privado por padrão"),
                        tr!("Tudo roda no computador; nada das suas planilhas é enviado."), None),
                    consulta::cartao("code", tr!("Macros e consultas"),
                        tr!("Macros em JavaScript (Alt+F11) e o OpenQuery para juntar dados."), None),
                    consulta::cartao("chart-bar", tr!("Gráficos completos"),
                        tr!("25 tipos, com dica do mouse, legenda clicável e elementos analíticos."), None),
                    consulta::cartao("history", tr!("Recuperação automática"),
                        tr!("Se o computador desligar, a planilha volta pela tela inicial."), None),
                    consulta::cartao("math-function", tr!("Fórmulas em português"),
                        tr!("SOMA, PROCV e centenas de outras, com ajuda na F1."), None),
                ], 3),
            ].spacing(17).into(),
            1 => column![
                consulta::cabecalho(tr!("Seus dados"),
                    tr!("Usados como autor dos comentários e nas propriedades dos arquivos que você salvar (.fshz e .xlsx)."),
                    Some(consulta::ilustracao(Ilustracao::Pessoa))),
                // Formulário: rótulo em cima do campo, dica embaixo.
                column![
                    campo_formulario(tr!("Nome"), tr!("Em branco, usa o do sistema"),
                        campo(trf!("Ex.: {}", autor_do_sistema()), &self.config.autor_nome, Evento::Nome, 440.)),
                    campo_formulario(tr!("Iniciais"), tr!("Aparecem nos comentários"),
                        campo(trf!("Ex.: {}", iniciais_de(&self.autor())), &self.config.autor_iniciais, Evento::Iniciais, 160.)),
                    campo_formulario(tr!("Organização"), tr!("Opcional, vai no arquivo"),
                        campo(tr!("Empresa ou equipe").into(), &self.config.autor_organizacao, Evento::Organizacao, 440.)),
                ].spacing(20),
                consulta::apagado(tr!("Nada disso sai do computador: fica nas preferências locais do FeatherSheet."), 11.5),
            ].spacing(17).into(),
            2 => column![
                consulta::cabecalho(tr!("Salvamento"),
                    tr!("Como o FeatherSheet protege o seu trabalho. A recuperação automática vem ligada."),
                    Some(consulta::ilustracao(Ilustracao::Salvamento))),
                linhas_salvamento(self),
                consulta::apagado(tr!("Resumo: o salvamento automático grava só arquivos .fshz já salvos. Um .xlsx, .csv ou .parquet aberto, ou uma planilha nova, fica protegido apenas pela recuperação automática até você usar Salvar como… em .fshz."), 11.5),
            ].spacing(17).into(),
            3 => column![
                consulta::cabecalho(tr!("Aparência"),
                    tr!("Idioma, tema e cor de destaque."),
                    Some(consulta::ilustracao(Ilustracao::Aparencia))),
                column![
                    linha(tr!("Idioma"), tr!("Interface, nomes das funções, separadores (; ou ,) e a Ajuda F1."), lista_idioma(self)),
                    linha(tr!("Tema"), tr!("Automático acompanha o sistema na abertura."), opcoes_tema(self)),
                    linha(tr!("Cor de destaque"), tr!("Botões, seleção e a marca do app."), amostras_acento(self)),
                ],
            ].spacing(17).into(),
            _ => column![
                consulta::cabecalho(tr!("Tudo pronto"),
                    tr!("Algumas portas de entrada para quando precisar. Esta janela continua no menu Ajuda, em Primeiros passos."),
                    Some(consulta::ilustracao(Ilustracao::Pronto))),
                consulta::grade(vec![
                    consulta::cartao("help", tr!("Ajuda (F1)"),
                        tr!("Guia de uso, todas as funções com exemplos e os atalhos."), Some(Msg::Ajuda)),
                    consulta::cartao("code", tr!("Editor de macros (Alt+F11)"),
                        tr!("Automatize tarefas com JavaScript."), Some(Msg::Macros)),
                    consulta::cartao("sparkles", tr!("O que há de novo"),
                        tr!("As novidades de cada versão do FeatherSheet."), Some(Msg::NovidadesAbrir)),
                    consulta::cartao("settings", tr!("Configurações"),
                        tr!("Todas as preferências, incluindo atalhos do teclado."), Some(Msg::Configuracoes)),
                ], 2),
            ].spacing(17).into(),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn iniciais_pegam_primeira_e_ultima_palavra() {
        assert_eq!(iniciais_de("marcelo de souza aragon"), "MA");
        assert_eq!(iniciais_de("Ana"), "A");
        assert_eq!(iniciais_de("  "), "");
    }

    #[test]
    fn passos_navegam_e_seus_dados_assinam_comentario_e_arquivo() {
        let mut app = App::default();
        for i in 0..PASSOS.len() {
            app.primeiros_passos_passo = i;
            let _: Element<'_, Msg> = app.view_primeiros_passos();
        }
        app.primeiros_passos_passo = 0;
        let _ = app.update(Msg::PrimeirosPassos(Evento::Anterior));
        assert_eq!(app.primeiros_passos_passo, 0);
        let _ = app.update(Msg::PrimeirosPassos(Evento::Proximo));
        let _ = app.update(Msg::PrimeirosPassos(Evento::Passo(99)));
        assert_eq!(app.primeiros_passos_passo, PASSOS.len() - 1);

        let _ = app.update(Msg::PrimeirosPassos(Evento::Nome("Ana Lima".into())));
        let _ = app.update(Msg::PrimeirosPassos(Evento::Iniciais("a l x y z".into())));
        let _ = app.update(Msg::PrimeirosPassos(Evento::Organizacao("Feather".into())));
        assert_eq!(app.autor(), "Ana Lima");
        assert_eq!(app.config.autor_iniciais, "ALXY");
        assert_eq!(app.iniciais_do_autor("Ana Lima"), "ALXY");
        assert_eq!(app.iniciais_do_autor("Bruno Reis"), "BR");

        // Arquivo sem autor ganha o da pessoa; autor de outro é preservado.
        app.carimbar_propriedades();
        assert_eq!(app.workbook.propriedades.autor, "Ana Lima");
        assert_eq!(app.workbook.propriedades.organizacao, "Feather");
        app.workbook.propriedades.autor = "Outra pessoa".into();
        app.config.autor_nome = "Ana L.".into();
        app.carimbar_propriedades();
        assert_eq!(app.workbook.propriedades.autor, "Outra pessoa");
        assert_eq!(app.workbook.propriedades.modificado_por, "Ana L.");

        // Sem nome, volta ao do sistema.
        app.config.autor_nome = "  ".into();
        assert_eq!(app.autor(), autor_do_sistema());
    }

    #[test]
    fn aviso_de_salvamento_automatico_so_para_documento_nao_coberto() {
        let mut app = App::default();
        app.config.auto_salvar = true;
        app.alterado = true;
        app.origem = Some(std::path::PathBuf::from("/tmp/dados.parquet"));
        app.avisar_fora_do_salvamento_automatico();
        let (aviso, grande) = app.faixa_salvamento.take().expect("avisou");
        assert!(aviso.contains(".parquet") && aviso.contains(".fshz") && !grande, "{aviso}");
        // Uma vez por documento (fechar a faixa não a traz de volta).
        app.avisar_fora_do_salvamento_automatico();
        assert!(app.faixa_salvamento.is_none());
        // Modelo aberto (origem .fshz, sem caminho): planilha ainda não salva.
        app.aviso_salvamento_de = None;
        app.origem = Some(std::path::PathBuf::from("/tmp/Fluxo de caixa.fshz"));
        app.avisar_fora_do_salvamento_automatico();
        let (aviso, _) = app.faixa_salvamento.take().expect("avisou");
        assert!(aviso.contains("ainda não salva"), "{aviso}");
        // Um .fshz salvo é coberto: nada a avisar.
        app.aviso_salvamento_de = None;
        app.origem = None;
        app.caminho = Some(std::path::PathBuf::from("/tmp/dados.fshz"));
        app.avisar_fora_do_salvamento_automatico();
        assert!(app.faixa_salvamento.is_none());
        let _: Element<'_, Msg> = app.view_faixa_salvamento();
    }

    #[test]
    fn primeira_abertura_mostra_passos_e_atualizacao_mostra_novidades() {
        let mut app = App::default();
        app.config.primeiros_passos_vistos = false;
        app.config.versao_vista.clear();
        app.config.existia = false;
        assert_eq!(app.boas_vindas_pendente(), Some(Auxiliar::PrimeirosPassos));
        // Já viu tudo desta versão: nada abre.
        assert_eq!(app.boas_vindas_pendente(), None);
        // Atualizou: as novidades abrem uma vez.
        app.config.versao_vista = "0.0.1".into();
        assert_eq!(app.boas_vindas_pendente(), Some(Auxiliar::Novidades));
        assert_eq!(app.boas_vindas_pendente(), None);
        // Atualizou da 0.0.2 (preferências antigas, sem os campos novos):
        // as novidades, não o assistente.
        let mut antigo = App::default();
        antigo.config.existia = true;
        antigo.config.primeiros_passos_vistos = false;
        antigo.config.versao_vista.clear();
        assert_eq!(antigo.boas_vindas_pendente(), Some(Auxiliar::Novidades));
        assert_eq!(antigo.boas_vindas_pendente(), None);
        // Nunca sozinha nos testes.
        app.config.primeiros_passos_vistos = false;
        assert_eq!(app.janela_de_boas_vindas(), None);
    }
}
