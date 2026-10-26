//! Proteger planilha (Página Inicial → Formatar ▾), como no Excel: a janela
//! com senha opcional e as 15 permissões, Desproteger com a senha, Bloquear
//! célula e o **portão** que confere cada ação na aba protegida.
//!
//! O portão tem duas metades. Antes: o que não entra no desfazer (inserir e
//! excluir linhas e colunas, formatar linha/coluna inteira) e começar a
//! digitar numa célula bloqueada são recusados na hora. Depois: o que a ação
//! deixou no desfazer é examinado pelo motor
//! (`Sheet::examinar_protecao`); se tocou em algo proibido, é desfeito na
//! hora e some do refazer. A regra mora no motor; aqui só a orquestração.
//!
//! A senha usa o hash do `<sheetProtection>` do Excel (SHA-512 com sal e
//! giros), para o `.xlsx` abrir protegido no Excel com a mesma senha.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use feathersheet_core::planilha::sheet::protecao::{Permissao, ProtecaoPlanilha, Recusa, SenhaPlanilha};
use sha2::{Digest, Sha512};
use std::collections::BTreeSet;

/// Giros do hash: o mesmo do Excel.
const GIROS: u32 = 100_000;
/// Arquivo malicioso não pode travar a interface com bilhões de giros.
const GIROS_MAXIMO: u32 = 10_000_000;

fn hash_excel(senha: &str, sal: &[u8], giros: u32) -> Vec<u8> {
    let utf16: Vec<u8> = senha.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut h = Sha512::new().chain_update(sal).chain_update(&utf16).finalize().to_vec();
    for i in 0..giros {
        h = Sha512::new().chain_update(&h).chain_update(i.to_le_bytes()).finalize().to_vec();
    }
    h
}

pub(super) fn criar_senha(senha: &str) -> Result<SenhaPlanilha, String> {
    let mut sal = [0_u8; 16];
    getrandom::getrandom(&mut sal).map_err(|_| tr!("O gerador criptográfico não está disponível."))?;
    Ok(SenhaPlanilha {
        algoritmo: "SHA-512".into(),
        hash: STANDARD.encode(hash_excel(senha, &sal, GIROS)),
        sal: STANDARD.encode(sal),
        giros: GIROS,
    })
}

pub(super) fn conferir(senha: &str, guardada: &SenhaPlanilha) -> Result<bool, String> {
    if !guardada.algoritmo.eq_ignore_ascii_case("SHA-512") {
        return Err(trf!("Senha protegida com {}, que o FeatherSheet ainda não confere.", guardada.algoritmo));
    }
    if guardada.giros > GIROS_MAXIMO { return Err(tr!("Proteção com número de giros inválido.").into()); }
    let sal = STANDARD.decode(&guardada.sal).map_err(|_| tr!("Sal da proteção inválido."))?;
    let hash = STANDARD.decode(&guardada.hash).map_err(|_| tr!("Hash da proteção inválido."))?;
    let obtido = hash_excel(senha, &sal, guardada.giros);
    Ok(obtido.len() == hash.len() && obtido.iter().zip(&hash).fold(0u8, |d, (a, b)| d | (a ^ b)) == 0)
}

#[derive(Debug, Clone)]
pub enum Evento {
    /// Proteger (janela) ou, protegida, Desproteger.
    Abrir,
    Senha(String),
    Confirmacao(String),
    Permitir(Permissao, bool),
    Aplicar,
    Fechar,
    /// Formatar ▾ → Bloquear célula: alterna o `locked` da seleção.
    AlternarBloqueio,
}

#[derive(Debug, Clone)]
pub(super) struct Estado {
    /// `true` = janela de Desproteger (só a senha).
    pub desproteger: bool,
    pub senha: String,
    pub confirmacao: String,
    pub permitidas: BTreeSet<Permissao>,
    pub erro: Option<String>,
}

impl App {
    fn aba_protegida(&self) -> Option<&feathersheet_core::planilha::sheet::Sheet> {
        self.workbook.aba(self.folha).map(|a| &a.sheet).filter(|s| s.protegida())
    }

    /// A metade "antes" do portão. `None`: pode seguir.
    pub(super) fn recusa_antes(&self, msg: &Msg) -> Option<Recusa> {
        let s = self.aba_protegida()?;
        let proibe = |p: Permissao| s.proibe(p).then(|| Recusa::sem_permissao(p));
        let a = self.area_selecionada();
        let linhas_da_selecao = Rect::new(a.r0, 0, a.r1, MAX_COLS - 1);
        let colunas_da_selecao = Rect::new(0, a.c0, MAX_ROWS - 1, a.c1);
        let excluir = |p: Permissao, area: &Rect| proibe(p).or_else(|| s.area_bloqueada(area).then(Recusa::celula_bloqueada));
        match msg {
            // Começar a digitar numa célula bloqueada: recusa já, como o Excel.
            Msg::Digitar(_) | Msg::Editar(_) | Msg::EditarNaCelula(..) | Msg::InserirQuebraLinha
            | Msg::EditarMultilinha | Msg::ListaDaColuna | Msg::AutoSoma
                if s.celula_bloqueada(self.linha, self.coluna) => Some(Recusa::celula_bloqueada()),
            Msg::Estrutura(e) | Msg::InserirMenu(e, _) => match e {
                Estrutura::InserirLinha => proibe(Permissao::InserirLinhas),
                Estrutura::InserirColuna => proibe(Permissao::InserirColunas),
                Estrutura::ExcluirLinha => excluir(Permissao::ExcluirLinhas, &linhas_da_selecao),
                Estrutura::ExcluirColuna => excluir(Permissao::ExcluirColunas, &colunas_da_selecao),
            },
            Msg::FaixaOpcao("linhas", op) => if op.starts_with("Excluir") { excluir(Permissao::ExcluirLinhas, &linhas_da_selecao) }
                else { proibe(Permissao::InserirLinhas) },
            Msg::FaixaOpcao("colunas", op) => if op.starts_with("Excluir") { excluir(Permissao::ExcluirColunas, &colunas_da_selecao) }
                else { proibe(Permissao::InserirColunas) },
            Msg::Celulas(celulas_dialogo::Evento::Abrir(_)) | Msg::Celulas(celulas_dialogo::Evento::Aplicar)
            | Msg::Topicos(topicos_ui::Evento::Agrupar | topicos_ui::Evento::Desagrupar | topicos_ui::Evento::Limpar
                | topicos_ui::Evento::SubtotalAbrir) | Msg::RemoverDuplicados | Msg::ApararEspacos
            | Msg::ValidacaoAbrir | Msg::CaixasInserir | Msg::TabelaAbrir | Msg::TabelaCriarPeloMenu => Some(Recusa::planilha_protegida()),
            // Formatar linha/coluna/aba inteira não entra no desfazer: recusar antes.
            Msg::Formatar(_) | Msg::EstiloMenu(..) | Msg::AlternarEstilo(_) | Msg::TamanhoFonteDelta(_)
            | Msg::Decimais(_) | Msg::LimparFormato | Msg::LimparHiperlinks | Msg::AlterarRecuo(_) | Msg::EstiloCelulaAplicar(_) | Msg::FormatarCelulasAbrir
            | Msg::AbrirSeletorCor(_) | Msg::Pincel | Msg::AplicarPincel | Msg::OrientacaoEscolher(_)
            | Msg::InclinarTexto | Msg::Mesclar | Msg::MesclarModo(_) | Msg::CondicionalAbrir
            | Msg::Protecao(Evento::AlternarBloqueio) => proibe(Permissao::FormatarCelulas),
            Msg::Desfazer | Msg::Refazer => {
                let refazer = matches!(msg, Msg::Refazer);
                s.celulas_do_proximo(refazer).iter()
                    .any(|a| s.area_bloqueada(&Rect::new(a[0], a[1], a[2], a[3])))
                    .then(|| Recusa(tr!("A planilha está protegida: o que seria desfeito mexe em células bloqueadas.").into()))
            }
            Msg::Selecionar(r, c) | Msg::Estender(r, c) => {
                let p = if s.celula_marcada_bloqueada(*r, *c) { Permissao::SelecionarBloqueadas } else { Permissao::SelecionarDesbloqueadas };
                proibe(p)
            }
            _ => None,
        }
    }

    /// Processa a mensagem com a aba protegida vigiada: a metade "depois"
    /// desfaz o que a ação fez e a proteção não deixa.
    pub(super) fn processar_protegido(&mut self, msg: Msg) -> Task<Msg> {
        if let Some(recusa) = self.recusa_antes(&msg) {
            self.status = recusa.0;
            return Task::none();
        }
        let marcas: Vec<(usize, usize)> = (0..self.workbook.len())
            .filter_map(|i| self.workbook.aba(i).filter(|a| a.sheet.protegida())
                .map(|a| (i, a.sheet.profundidade_do_historico())))
            .collect();
        let tarefa = self.processar_mensagem(msg);
        for (i, marca) in marcas {
            let recusa = self.workbook.aba(i).and_then(|a| a.sheet.examinar_protecao(marca));
            if let Some(r) = recusa {
                self.workbook.editar(i, |s| s.desfazer_ate(marca));
                self.status = r.0;
            }
        }
        tarefa
    }

    pub(super) fn evento_protecao(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => {
                if !self.confirmar() { return Task::none(); }
                let Some(aba) = self.workbook.aba(self.folha) else { return Task::none() };
                match &aba.sheet.protecao {
                    Some(p) if p.senha.is_none() => {
                        self.workbook.editar(self.folha, |s| { s.desproteger(); });
                        self.alterado = true;
                        self.status = tr!("Planilha desprotegida.").into();
                    }
                    Some(_) => self.protecao_modal = Some(Estado { desproteger: true, senha: String::new(),
                        confirmacao: String::new(), permitidas: BTreeSet::new(), erro: None }),
                    None => self.protecao_modal = Some(Estado { desproteger: false, senha: String::new(),
                        confirmacao: String::new(), permitidas: Permissao::padrao(), erro: None }),
                }
                self.edicao_menu = None;
                self.contexto = None;
            }
            Evento::Senha(t) => if let Some(e) = &mut self.protecao_modal { e.senha = t; e.erro = None; },
            Evento::Confirmacao(t) => if let Some(e) = &mut self.protecao_modal { e.confirmacao = t; e.erro = None; },
            Evento::Permitir(p, v) => if let Some(e) = &mut self.protecao_modal {
                if v { e.permitidas.insert(p); } else { e.permitidas.remove(&p); }
            },
            Evento::Aplicar => self.aplicar_protecao(),
            Evento::Fechar => self.protecao_modal = None,
            Evento::AlternarBloqueio => {
                let bloqueada = self.workbook.aba(self.folha)
                    .is_some_and(|a| a.sheet.celula_marcada_bloqueada(self.linha, self.coluna));
                self.aplicar_estilo_menu("locked", Campo::Logico(!bloqueada));
                self.status = if bloqueada { tr!("Células desbloqueadas: ficam editáveis com a planilha protegida.") }
                    else { tr!("Células bloqueadas.") }.into();
            }
        }
        Task::none()
    }

    fn aplicar_protecao(&mut self) {
        let Some(e) = self.protecao_modal.clone() else { return };
        if e.desproteger {
            let senha = self.aba_protegida().and_then(|s| s.protecao.as_ref()).and_then(|p| p.senha.clone());
            let ok = match senha { Some(g) => conferir(&e.senha, &g), None => Ok(true) };
            match ok {
                Ok(true) => {
                    self.workbook.editar(self.folha, |s| { s.desproteger(); });
                    self.protecao_modal = None;
                    self.alterado = true;
                    self.status = tr!("Planilha desprotegida.").into();
                }
                Ok(false) => if let Some(m) = &mut self.protecao_modal { m.erro = Some(tr!("Senha incorreta.").into()); },
                Err(erro) => if let Some(m) = &mut self.protecao_modal { m.erro = Some(erro); },
            }
            return;
        }
        if e.senha != e.confirmacao {
            if let Some(m) = &mut self.protecao_modal { m.erro = Some(tr!("A confirmação não confere com a senha.").into()); }
            return;
        }
        let senha = if e.senha.is_empty() { None } else {
            match criar_senha(&e.senha) {
                Ok(s) => Some(s),
                Err(erro) => { if let Some(m) = &mut self.protecao_modal { m.erro = Some(erro); } return; }
            }
        };
        let protecao = ProtecaoPlanilha { senha, permitidas: e.permitidas };
        self.workbook.editar(self.folha, |s| { s.proteger(protecao); });
        self.protecao_modal = None;
        self.alterado = true;
        self.status = tr!("Planilha protegida. Só as células desbloqueadas (menu Formatar, Bloquear célula) aceitam digitação.").into();
    }

    fn view_protecao(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let e = self.protecao_modal.as_ref().expect("janela de proteção aberta");
        let campo = |dica: &str, valor: &str, msg: fn(String) -> Msg| {
            pelicano::text_input(dica.to_owned(), valor.to_owned()).secure(true).on_input(msg)
                .on_submit(Msg::Protecao(Evento::Aplicar)).padding(7).size(13)
        };
        let mut painel = pelicano::column(Vec::<Element<_>>::new()).spacing(10);
        if e.desproteger {
            painel = painel.push(campo(tr!("Senha"), &e.senha, |t| Msg::Protecao(Evento::Senha(t))));
        } else {
            let mut lista = pelicano::column(Vec::<Element<_>>::new()).spacing(7);
            for (p, rotulo, _) in Permissao::TODAS {
                lista = lista.push(pelicano::checkbox(rotulo, e.permitidas.contains(&p))
                    .on_toggle(move |v| Msg::Protecao(Evento::Permitir(p, v))).size(12.5));
            }
            painel = painel
                .push(text(tr!("Senha para desproteger (opcional)")).size(12).color(COR_TEXTO_MUTED()))
                .push(campo(tr!("Sem senha"), &e.senha, |t| Msg::Protecao(Evento::Senha(t))))
                .push(campo(tr!("Confirme a senha"), &e.confirmacao, |t| Msg::Protecao(Evento::Confirmacao(t))))
                .push(text(tr!("Permitir que todos os usuários desta planilha possam")).size(12).color(COR_TEXTO_MUTED()))
                .push(container(container(super::dialogo_interno::dlg::rolagem(lista.width(Length::FILL))).max_height(190.0))
                    .width(Length::FILL).padding([8, 10]).style(super::dialogo_interno::dlg::estilo_bloco));
        }
        if let Some(erro) = &e.erro {
            painel = painel.push(row![feather_ui::icones::icone("info-circle", 15.0),
                text(erro.clone()).size(12).color(COR_ERRO())]
                .spacing(6).align(pelicano::Align::Center));
        }
        let (titulo, sub) = if e.desproteger {
            (tr!("Desproteger planilha"), tr!("Digite a senha usada para proteger a aba."))
        } else {
            (tr!("Proteger planilha"), tr!("Células bloqueadas não mudam. Desbloqueie antes as que devem aceitar \
                digitação (menu Formatar, Bloquear célula)."))
        };
        self.dialogo(titulo, painel)
            .icone("lock")
            .subtitulo(sub)
            .largura(460.0)
            .ao_fechar(Msg::Protecao(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(Msg::Protecao(Evento::Fechar)))
            .primario("OK", Some(Msg::Protecao(Evento::Aplicar)))
    }

    pub(super) fn sobrepor_protecao<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.protecao_modal.is_none() { return base; }
        self.view_protecao().sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn raw(app: &App, r: u32, c: u32) -> String { app.workbook.aba(0).unwrap().sheet.get_raw(r, c) }

    fn proteger(app: &mut App, senha: &str) {
        let _ = app.update(Msg::Protecao(Evento::Abrir));
        let _ = app.update(Msg::Protecao(Evento::Senha(senha.into())));
        let _ = app.update(Msg::Protecao(Evento::Confirmacao(senha.into())));
        let _ = app.update(Msg::Protecao(Evento::Aplicar));
        assert!(app.workbook.aba(0).unwrap().sheet.protegida(), "{}", app.status);
    }

    #[test]
    fn senha_no_formato_do_excel_confere_e_recusa_errada() {
        let s = criar_senha("Fluxo 2026").unwrap();
        assert!(conferir("Fluxo 2026", &s).unwrap());
        assert!(!conferir("fluxo 2026", &s).unwrap());
        assert_eq!(STANDARD.decode(&s.hash).unwrap().len(), 64);
        let mut ruim = s.clone();
        ruim.giros = u32::MAX;
        assert!(conferir("x", &ruim).is_err(), "giros absurdos não travam a tela");
    }

    #[test]
    fn protegida_so_a_celula_desbloqueada_aceita_e_colar_e_desfeito() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "fixo".into());
        // B1 desbloqueada antes de proteger.
        let _ = app.update(Msg::Selecionar(0, 1));
        let _ = app.update(Msg::Protecao(Evento::AlternarBloqueio));
        proteger(&mut app, "");
        let _ = app.update(Msg::Selecionar(0, 0));
        let _ = app.update(Msg::Digitar("x".into()));
        assert!(app.status.contains("protegida"), "{}", app.status);
        // Colar por cima de A1:B1 é desfeito inteiro.
        let _ = app.update(Msg::Colar("a\tb".into()));
        assert_eq!((raw(&app, 0, 0), raw(&app, 0, 1)), ("fixo".into(), "".into()), "{}", app.status);
        // Na B1 pode.
        let _ = app.update(Msg::Selecionar(0, 1));
        let _ = app.update(Msg::Colar("livre".into()));
        assert_eq!(raw(&app, 0, 1), "livre");
        // Inserir linha não está permitido no padrão.
        let _ = app.update(Msg::Estrutura(Estrutura::InserirLinha));
        assert_eq!(raw(&app, 0, 0), "fixo");
        let _ = app.update(Msg::Formatar(Formatacao::Negrito));
        assert!(app.status.contains("formatar células"), "{}", app.status);
    }

    #[test]
    fn desproteger_pede_a_senha() {
        let mut app = App::default();
        proteger(&mut app, "segredo");
        let _ = app.update(Msg::Protecao(Evento::Abrir));
        assert!(app.protecao_modal.as_ref().is_some_and(|e| e.desproteger));
        let _ = app.update(Msg::Protecao(Evento::Senha("errada".into())));
        let _ = app.update(Msg::Protecao(Evento::Aplicar));
        assert!(app.workbook.aba(0).unwrap().sheet.protegida());
        let _ = app.update(Msg::Protecao(Evento::Senha("segredo".into())));
        let _ = app.update(Msg::Protecao(Evento::Aplicar));
        assert!(!app.workbook.aba(0).unwrap().sheet.protegida());
    }

    #[test]
    fn formula_oculta_some_da_barra_so_protegida() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "=1+1".into());
        let _ = app.update(Msg::Selecionar(0, 0));
        app.aplicar_estilo_menu("hidden", Campo::Logico(true));
        let _ = app.update(Msg::Selecionar(0, 0));
        assert_eq!(app.edicao, "=1+1");
        proteger(&mut app, "");
        let _ = app.update(Msg::Selecionar(0, 0));
        assert_eq!(app.edicao, "");
    }
}
