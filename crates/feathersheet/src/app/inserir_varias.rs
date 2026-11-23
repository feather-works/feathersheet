//! "Inserir linhas acima/abaixo…" e "Inserir colunas à esquerda/direita…": uma caixa pede quantas
//! (ideia do fadi123go na #164). Vem preenchida com o tamanho da seleção, como o Excel insere.
use super::*;

const ID_CAMPO: &str = "feathersheet.inserir-varias";
const MAXIMO: u32 = 10_000;

#[derive(Debug, Clone)]
pub enum Evento { Abrir(Estrutura, bool), Lado(bool), Valor(String), Aplicar, Fechar }

#[derive(Debug, Clone)]
pub(super) struct Estado { pub acao: Estrutura, pub depois: bool, pub valor: String, pub erro: bool }

fn ler(texto: &str) -> Option<u32> {
    texto.trim().parse::<u32>().ok().filter(|n| (1..=MAXIMO).contains(n))
}

impl App {
    pub(super) fn evento_inserir_varias(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir(acao, depois) => {
                if !self.confirmar() { return Task::none(); }
                let a = self.area_selecionada();
                let n = if acao == Estrutura::InserirColuna { a.c1 - a.c0 + 1 } else { a.r1 - a.r0 + 1 };
                self.contexto = None;
                self.inserir_varias = Some(Estado { acao, depois, valor: n.min(MAXIMO).to_string(), erro: false });
                return Task::focus(ID_CAMPO);
            }
            Evento::Lado(d) => if let Some(e) = &mut self.inserir_varias { e.depois = d; },
            Evento::Valor(v) => if let Some(e) = &mut self.inserir_varias { e.valor = v; e.erro = false; },
            Evento::Aplicar => if let Some(e) = self.inserir_varias.clone() {
                match ler(&e.valor) {
                    Some(n) => { self.inserir_varias = None; self.inserir_menu_n(e.acao, e.depois, n); }
                    None => if let Some(e) = &mut self.inserir_varias { e.erro = true; },
                }
            },
            Evento::Fechar => self.inserir_varias = None,
        }
        Task::none()
    }

    pub(super) fn sobrepor_inserir_varias<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some(s) = &self.inserir_varias else { return base };
        let coluna = s.acao == Estrutura::InserirColuna;
        let (titulo, antes, depois) = if coluna {
            (tr!("Inserir colunas"), tr!("À esquerda"), tr!("À direita"))
        } else { (tr!("Inserir linhas"), tr!("Acima"), tr!("Abaixo")) };
        use super::configuracoes_ui::opcao;
        let lado = row![
            opcao(antes, if coluna { "arrow-left" } else { "arrow-up" }, !s.depois, Msg::InserirVarias(Evento::Lado(false))),
            opcao(depois, if coluna { "arrow-right" } else { "arrow-down" }, s.depois, Msg::InserirVarias(Evento::Lado(true))),
        ].spacing(3);
        let campo = text_input("", &s.valor).id(ID_CAMPO)
            .on_input(|v| Msg::InserirVarias(Evento::Valor(v)))
            .on_submit(Msg::InserirVarias(Evento::Aplicar))
            .padding(9).width(Length::Fixed(120.0));
        let mut painel = pelicano::column![
            row![text(tr!("Quantas")).size(13), campo].spacing(12).align(pelicano::Align::Center),
            lado,
        ].spacing(10);
        if s.erro {
            painel = painel.push(text(trf!("Digite um número inteiro de 1 a {MAXIMO}.")).size(12).color(COR_ERRO()));
        }
        self.dialogo(titulo, painel)
            .icone(if coluna { "arrows-horizontal" } else { "arrows-vertical" })
            .largura(380.0)
            .ao_fechar(Msg::InserirVarias(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(Msg::InserirVarias(Evento::Fechar)))
            .primario("OK", Some(Msg::InserirVarias(Evento::Aplicar)))
            .sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn insere_quantas_linhas_pedir_abaixo_da_celula() {
        let mut app = App::default();
        let _ = app.update(Msg::Selecionar(2, 0));
        app.workbook.escrever_celula(0, 3, 0, "depois".into());
        let _ = app.update(Msg::InserirVarias(Evento::Abrir(Estrutura::InserirLinha, true)));
        assert_eq!(app.inserir_varias.as_ref().map(|e| e.valor.as_str()), Some("1"), "vem com o tamanho da seleção");
        let _ = app.update(Msg::InserirVarias(Evento::Valor("0".into())));
        let _ = app.update(Msg::InserirVarias(Evento::Aplicar));
        assert!(app.inserir_varias.as_ref().is_some_and(|e| e.erro), "zero é recusado");
        let _ = app.update(Msg::InserirVarias(Evento::Valor("5".into())));
        let _ = app.update(Msg::InserirVarias(Evento::Aplicar));
        assert!(app.inserir_varias.is_none());
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.get_raw(8, 0), "depois", "cinco linhas entraram abaixo da linha 3");
        assert_eq!(sheet.get_raw(3, 0), "");
    }

    #[test]
    fn abaixo_de_um_bloco_entra_depois_dele_todo() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 7, 0, "depois".into());
        let _ = app.update(Msg::SelecionarLinha(4));
        let _ = app.update(Msg::EstenderLinha(6));
        app.inserir_menu_n(Estrutura::InserirLinha, true, 3);
        // Linhas 5–7 selecionadas (ativa na 5): as três entram abaixo da 7, não no meio do bloco.
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(10, 0), "depois");
    }
}
