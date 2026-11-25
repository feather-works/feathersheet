//! Macros ▸ Gravar macro, como o gravador do Excel (Desenvolvedor ▸ Gravar Macro): uma caixa pede o nome e se
//! as referências são relativas; enquanto grava, o rodapé mostra "● Gravando" com o Parar. Cada transação
//! fechada em qualquer aba (o que o Ctrl+Z desfaz) é colhida depois de cada mensagem e traduzida em JavaScript
//! (`feathersheet_macros::gravador`); inserir e excluir linhas ou colunas entram à parte, porque trocam o
//! histórico da aba pela foto da pasta. Ao parar, a macro vira um módulo novo e abre no editor.
// shortcut: criar, renomear ou apagar aba, a seleção (Select do Excel) e o que não passa pelo Ctrl+Z (filtro,
// zoom, congelar) não são gravados; o que o tradutor não sabe escrever vira comentário na macro.
use super::*;
use feathersheet_macros::gravador::{self, Alvo};
use std::collections::HashMap;

const ID_CAMPO: &str = "feathersheet.gravar-macro";

#[derive(Debug, Clone)]
pub enum Evento { Abrir, Nome(String), Relativas(bool), Comecar, Fechar, Parar }

#[derive(Debug, Clone)]
pub(super) struct Dialogo { pub nome: String, pub relativas: bool, pub erro: Option<String> }

pub(super) struct Gravacao {
    pub nome: String,
    relativas: bool,
    /// A aba em que a gravação começou: as outras saem como `sheets("Nome")`.
    aba: String,
    /// Célula ativa no começo (referências relativas).
    ativa: (u32, u32),
    linhas: Vec<String>,
    /// Transações já vistas por aba (`Sheet::edicoes_novas`), pelo nome.
    vistas: HashMap<String, u64>,
}

/// Nome de macro como o Excel aceita: começa com letra, só letras, dígitos e `_`.
fn nome_valido(nome: &str) -> bool {
    let mut c = nome.chars();
    c.next().is_some_and(char::is_alphabetic) && c.all(|x| x.is_alphanumeric() || x == '_') && nome.chars().count() <= 64
}

impl App {
    pub(super) fn evento_gravador(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => {
                if self.gravacao.is_some() { return self.update(Msg::GravarMacro(Evento::Parar)); }
                let mut n = 1;
                while self.macro_catalogo.obter(&format!("Macro{n}")).is_some() { n += 1; }
                self.gravar_dialogo = Some(Dialogo { nome: format!("Macro{n}"), relativas: self.gravar_relativas, erro: None });
                return Task::focus(ID_CAMPO);
            }
            Evento::Nome(v) => if let Some(d) = &mut self.gravar_dialogo { d.nome = v; d.erro = None; },
            Evento::Relativas(v) => {
                self.gravar_relativas = v;
                if let Some(d) = &mut self.gravar_dialogo { d.relativas = v; }
            }
            Evento::Comecar => if let Some(d) = self.gravar_dialogo.clone() {
                let nome = d.nome.trim().to_string();
                let erro = if !nome_valido(&nome) {
                    Some(tr!("O nome começa com letra e tem só letras, números e _ (sem espaços).").to_string())
                } else if self.macro_catalogo.obter(&nome).is_some() {
                    Some(trf!("Já existe um módulo chamado {nome}."))
                } else { None };
                if let Some(e) = erro {
                    if let Some(d) = &mut self.gravar_dialogo { d.erro = Some(e); }
                    return Task::none();
                }
                self.confirmar();
                self.gravar_dialogo = None;
                let vistas = (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i))
                    .map(|a| (a.sheet.name.clone(), a.sheet.edicoes_novas())).collect();
                let aba = self.workbook.aba(self.folha).map(|a| a.sheet.name.clone()).unwrap_or_default();
                self.gravacao = Some(Gravacao { nome: nome.clone(), relativas: d.relativas, aba, ativa: (self.linha, self.coluna), linhas: Vec::new(), vistas });
                self.status = trf!("Gravando a macro {nome}.");
            },
            Evento::Fechar => self.gravar_dialogo = None,
            Evento::Parar => return self.parar_gravacao(),
        }
        Task::none()
    }

    /// O que as abas fizeram desde a última colheita (chamado depois de cada mensagem enquanto grava).
    pub(super) fn colher_gravacao(&mut self) {
        let Some(g) = &mut self.gravacao else { return };
        for i in 0..self.workbook.len() {
            let Some(a) = self.workbook.aba(i) else { continue };
            let agora = a.sheet.edicoes_novas();
            let antes = g.vistas.get(&a.sheet.name).copied().unwrap_or(0);
            if agora == antes { continue; }
            // Histórico trocado pela foto de inserir/excluir (contador menor): tudo o que está nele é novo.
            let n = if agora > antes { agora - antes } else { agora };
            g.vistas.insert(a.sheet.name.clone(), agora);
            let objeto = if a.sheet.name == g.aba { "sheet".to_string() } else { format!("sheets({:?})", a.sheet.name) };
            let relativa_a = (g.relativas && a.sheet.name == g.aba).then_some(g.ativa);
            for t in a.sheet.transacoes_para_gravar(usize::try_from(n).unwrap_or(usize::MAX)) {
                g.linhas.extend(gravador::traduzir(&Alvo { objeto: &objeto, relativa_a }, &t));
            }
        }
    }

    /// Inserir/excluir linhas ou colunas na aba ativa (`at` a partir de 0), na ordem certa entre as outras ações.
    pub(super) fn gravar_estrutura(&mut self, coluna: bool, removendo: bool, at: u32, n: u32) {
        if self.gravacao.is_none() { return; }
        self.colher_gravacao();
        let Some(g) = &mut self.gravacao else { return };
        let Some(a) = self.workbook.aba(self.folha) else { return };
        let objeto = if a.sheet.name == g.aba { "sheet".to_string() } else { format!("sheets({:?})", a.sheet.name) };
        let metodo = match (coluna, removendo) { (false, false) => "insertRow", (false, true) => "deleteRow", (true, false) => "insertColumn", (true, true) => "deleteColumn" };
        let onde = if coluna { format!("{:?}", feathersheet_core::celulas::refs::col_name(at)) } else { (at + 1).to_string() };
        g.linhas.push(format!("{objeto}.{metodo}({onde}, {n});"));
        g.vistas.insert(a.sheet.name.clone(), a.sheet.edicoes_novas());
    }

    fn parar_gravacao(&mut self) -> Task<Msg> {
        self.colher_gravacao();
        let Some(g) = self.gravacao.take() else { return Task::none() };
        if g.linhas.is_empty() {
            self.status = trf!("Nada foi gravado na macro {}.", g.nome);
            return Task::none();
        }
        let codigo = format!("{}{}\n", gravador::cabecalho(&g.nome, g.relativas), g.linhas.join("\n"));
        let mut catalogo = self.macro_catalogo.clone();
        catalogo.macros.push(feather_macros::Macro { name: g.nome.clone(), code: codigo.clone(), auto: false });
        if !self.persistir_modulos(catalogo, &trf!("Macro {} gravada", g.nome)) { return Task::none(); }
        // Abre a gravada no editor, se o que está nele já foi guardado (senão fica na lista de módulos).
        let guardado = self.macro_catalogo.obter(self.macro_nome.trim()).is_some_and(|m| m.code == self.macro_codigo.text());
        if guardado {
            self.macro_nome = g.nome.clone();
            self.macro_codigo = pelicano::Content::with_text(&codigo);
            self.macro_visual.documento = format!("modulo:{}", g.nome);
        }
        if self.janelas.aberta(Auxiliar::Macros) { Task::none() } else { self.alternar(Auxiliar::Macros) }
    }

    /// "● Gravando Macro1 · Parar" no rodapé.
    pub(super) fn view_gravando(&self) -> Element<'_, Msg> {
        let Some(g) = &self.gravacao else { return Space::new(0, 0.0).into() };
        button(row![
            text("●").size(12).color(COR_ERRO()),
            text(trf!("Gravando {} · Parar", g.nome)).size(11),
        ].spacing(5).align(pelicano::Align::Center))
            .padding([3, 8]).style(estilo_botao_pequeno(false))
            .on_press(Msg::GravarMacro(Evento::Parar))
            .into()
    }

    pub(super) fn sobrepor_gravar_macro<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some(d) = &self.gravar_dialogo else { return base };
        let campo = text_input("", &d.nome).id(ID_CAMPO)
            .on_input(|v| Msg::GravarMacro(Evento::Nome(v)))
            .on_submit(Msg::GravarMacro(Evento::Comecar))
            .padding(9).width(Length::Fixed(220.0));
        let relativas = pelicano::checkbox(tr!("Usar referências relativas"), d.relativas)
            .on_toggle(|v| Msg::GravarMacro(Evento::Relativas(v))).size(13);
        let mut painel = pelicano::column![
            row![text(tr!("Nome da macro")).size(13), campo].spacing(12).align(pelicano::Align::Center),
            relativas,
            text(tr!("Relativas: a macro faz o mesmo a partir da célula em que estiver quando rodar.")).size(12).color(COR_TEXTO_MUTED()),
        ].spacing(10);
        if let Some(e) = &d.erro {
            painel = painel.push(text(e.clone()).size(12).color(COR_ERRO()));
        }
        self.dialogo(tr!("Gravar macro"), painel)
            .icone("player-record")
            .largura(420.0)
            .ao_fechar(Msg::GravarMacro(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(Msg::GravarMacro(Evento::Fechar)))
            .primario(tr!("Gravar"), Some(Msg::GravarMacro(Evento::Comecar)))
            .sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn grava_o_que_se_faz_e_vira_modulo_que_refaz() {
        let mut app = App::default();
        let _ = app.update(Msg::GravarMacro(Evento::Abrir));
        assert_eq!(app.gravar_dialogo.as_ref().map(|d| d.nome.as_str()), Some("Macro1"));
        let _ = app.update(Msg::GravarMacro(Evento::Nome("tem espaço".into())));
        let _ = app.update(Msg::GravarMacro(Evento::Comecar));
        assert!(app.gravar_dialogo.as_ref().is_some_and(|d| d.erro.is_some()), "nome com espaço é recusado");
        let _ = app.update(Msg::GravarMacro(Evento::Nome("Relatorio".into())));
        let _ = app.update(Msg::GravarMacro(Evento::Comecar));
        assert!(app.gravacao.is_some());
        app.workbook.editar(0, |s| { s.abrir_transacao(); s.set_cell(0, 0, "Total"); s.set_cell(1, 0, "=2*21"); s.fechar_transacao(); });
        let _ = app.update(Msg::Selecionar(4, 0));
        app.gravar_estrutura(false, false, 2, 1);
        let _ = app.update(Msg::GravarMacro(Evento::Parar));
        assert!(app.gravacao.is_none());
        let codigo = app.macro_catalogo.obter("Relatorio").map(|m| m.code.clone()).expect("o módulo foi criado");
        assert!(codigo.contains("sheet.set(\"A1\", \"Total\");") && codigo.contains("sheet.setFormula(\"A2\", \"=2*21\");"), "{codigo}");
        assert!(codigo.contains("sheet.insertRow(3, 1);"), "{codigo}");
        // Rodada numa pasta limpa, a macro refaz as células.
        let mut limpa = feathersheet_core::planilha::workbook::Workbook::new();
        let r = feathersheet_macros::executar(&mut limpa, 0, &codigo);
        assert!(r.erro.is_none(), "{:?}", r.erro);
        assert_eq!(limpa.aba(0).unwrap().sheet.get_raw(0, 0), "Total");
    }
}
