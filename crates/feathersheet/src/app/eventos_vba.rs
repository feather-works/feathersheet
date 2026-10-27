//! Eventos convertidos do VBA rodando sozinhos, como no Excel:
//! `Worksheet_Change` (aoMudar) depois de cada mudança feita pela pessoa na
//! aba, com o endereço mudado em `valores.intervalo` (o `Target`), e
//! `Worksheet_Activate` (aoAtivar) ao trocar para a aba. O `Workbook_Open`
//! continua à mão: rodar código ao abrir um arquivo é o caso mais arriscado.
//!
//! O que uma macro escreve não dispara evento (seria laço sem fim).
use super::*;

/// Um evento instalado: aba, evento ("aoMudar"/"aoAtivar") e o módulo.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct EventoVba {
    pub aba: String,
    pub evento: String,
    pub modulo: String,
}

/// O antes de uma mensagem, para comparar depois.
pub(super) struct Antes {
    folha: usize,
    aba_id: Option<u32>,
    versao: Option<u32>,
}

impl App {
    pub(super) fn antes_dos_eventos(&self) -> Option<Antes> {
        if self.eventos_vba.is_empty() { return None; }
        let aba = self.workbook.aba(self.folha);
        Some(Antes { folha: self.folha, aba_id: aba.map(|a| a.id), versao: aba.map(|a| a.versao) })
    }

    /// Depois de tratar a mensagem: dispara aoMudar e aoAtivar.
    pub(super) fn disparar_eventos(&mut self, antes: Antes) {
        if std::mem::take(&mut self.macro_rodou) { return; }
        let Some(aba) = self.workbook.aba(antes.folha) else { return };
        let nome_antes = aba.sheet.name.clone();
        // Mudou célula na mesma aba (o id confere: não foi arquivo novo).
        if Some(aba.id) == antes.aba_id && Some(aba.versao) != antes.versao {
            if let Some(area) = aba.sheet.area_da_ultima_transacao() {
                let alvo = feathersheet_core::celulas::refs::format_ref(&area);
                self.rodar_evento(&nome_antes, "aoMudar", Some(&alvo));
            }
        }
        if self.folha != antes.folha {
            if let Some(nova) = self.workbook.aba(self.folha).map(|a| a.sheet.name.clone()) {
                self.rodar_evento(&nova, "aoAtivar", None);
            }
        }
        self.macro_rodou = false;
    }

    fn rodar_evento(&mut self, aba: &str, evento: &str, alvo: Option<&str>) {
        if self.macros_bloqueadas { return; }
        let Some(e) = self.eventos_vba.iter()
            .find(|e| e.evento == evento && e.aba.eq_ignore_ascii_case(aba)).cloned() else { return };
        let Some(m) = self.macro_catalogo.obter(&e.modulo) else { return };
        let mut codigo = m.code.clone();
        if let Some(alvo) = alvo {
            codigo = format!("valores.intervalo={};\n{codigo}",
                serde_json::to_string(alvo).unwrap_or_default());
        }
        // Nos testes não: o diário é global e outro teste do console o confere.
        if !cfg!(test) {
            feathersheet_core::diario::anotar(trf!("[evento] {evento} da aba {aba}{}",
                alvo.map(|a| trf!(" em {a}")).unwrap_or_default()));
        }
        if let Some(r) = self.rodar_macro(historico_macros::GatilhoMacro::Evento {
            evento: evento.to_owned(), aba: aba.to_owned(), intervalo: alvo.map(str::to_owned),
        }, &e.modulo, &codigo, Vec::new()) {
            if let Some(erro) = r.erro {
                self.status = trf!("Evento {evento} da aba {aba}: {erro}. Escritas revertidas.");
            }
        }
    }
}

impl App {
    /// Arquivo aberto do disco com macros que rodam sozinhas (eventos de aba,
    /// botões ou formas com macro): ficam bloqueadas até a pessoa ativar, como no Excel.
    pub(super) fn bloquear_macros_do_arquivo(&mut self) {
        let abas = || (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i));
        let botoes = abas().any(|a| a.sheet.bt.as_deref().is_some_and(|t| !matches!(t.trim(), "" | "[]")));
        // Uma forma transparente sobre a grade rodaria a macro no primeiro clique (#49).
        let formas = abas().any(|a| feathersheet_io::objetos_aba::ler_da_aba(&a.sheet)
            .objetos.iter().any(|o| o.macro_nome.is_some()));
        self.macros_bloqueadas = botoes || formas || !self.eventos_vba.is_empty();
        self.aviso_macros = self.macros_bloqueadas;
    }

    /// Bloqueadas: reabre o "Ativar macros?" e devolve `false`.
    pub(super) fn macros_liberadas(&mut self) -> bool {
        if self.macros_bloqueadas { self.aviso_macros = true; }
        !self.macros_bloqueadas
    }

    pub(super) fn sobrepor_aviso_macros<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        use super::dialogo_interno::dlg;
        if !self.aviso_macros { return base; }
        let corpo = text(tr!("Este arquivo tem macros que rodam sozinhas (ao mudar células, ao trocar de aba ou ao clicar em botões). Elas podem alterar a planilha e gravar arquivos. Ative só se você confia na origem do arquivo."))
            .size(12.5).line_height(1.4);
        self.dialogo(tr!("Ativar macros?"), corpo)
            .icone("alert-triangle")
            .largura(480.0)
            .botao(dlg::botao_primario(tr!("Ativar macros"), Some(Msg::AtivarMacros)))
            .botao(dlg::botao_secundario(tr!("Manter desativadas"), Some(Msg::AvisoMacrosFechar)))
            .ao_fechar(Msg::AvisoMacrosFechar)
            .sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn app_com_evento(evento: &str, codigo: &str) -> App {
        let mut app = App::default();
        app.macro_catalogo = Default::default();
        app.macro_catalogo.macros.push(feather_macros::Macro {
            name: "Evento_teste".into(), code: codigo.into(), auto: false });
        app.eventos_vba = vec![EventoVba { aba: "Planilha1".into(), evento: evento.into(), modulo: "Evento_teste".into() }];
        app
    }

    #[test]
    fn mudar_celula_roda_o_evento_com_o_endereco() {
        // Como o conversor gera: a função se chama com valores.intervalo.
        let mut app = app_com_evento("aoMudar", r#"
            function Worksheet_Change(Target) {
              if (sheet.intersect(Target, "B19") === null) return;
              sheet.set("C1", "mudou " + Target);
            }
            Worksheet_Change(valores.intervalo || sheet.selection().address);"#);
        app.update(Msg::Selecionar(18, 1));
        app.update(Msg::Editar("NÃO".into()));
        app.update(Msg::Confirmar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 2), "mudou B19");
        // Fora de B19 o evento roda e não faz nada; a escrita da macro não
        // dispara o evento de novo.
        app.update(Msg::Selecionar(4, 4));
        app.update(Msg::Editar("x".into()));
        app.update(Msg::Confirmar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 2), "mudou B19");
    }

    #[test]
    fn trocar_de_aba_roda_o_ativar() {
        let mut app = app_com_evento("aoAtivar", "sheet.set('A1', 'ativou');");
        app.eventos_vba[0].aba = "Dados".into();
        let outra = app.workbook.criar_aba("Dados");
        app.update(Msg::Aba(outra));
        assert_eq!(app.workbook.aba(outra).unwrap().sheet.get_raw(0, 0), "ativou");
    }

    /// #59: o que a macro de evento escreve numa aba protegida é desfeito,
    /// como o que a pessoa ou um botão de macro escreveria.
    #[test]
    fn evento_nao_escreve_em_aba_protegida() {
        use feathersheet_core::planilha::sheet::protecao::ProtecaoPlanilha;
        let mut app = app_com_evento("aoAtivar", "sheet.set('A1', 'ativou');");
        app.eventos_vba[0].aba = "Dados".into();
        let outra = app.workbook.criar_aba("Dados");
        app.workbook.editar(outra, |s| { s.proteger(ProtecaoPlanilha { senha: None, permitidas: Default::default() }); });
        app.update(Msg::Aba(outra));
        assert_eq!(app.workbook.aba(outra).unwrap().sheet.get_raw(0, 0), "", "célula bloqueada não muda");
    }

    /// Arquivo aberto com evento: nada roda até "Ativar macros".
    #[test]
    fn evento_de_arquivo_aberto_so_roda_depois_de_ativar() {
        let mut app = app_com_evento("aoAtivar", "sheet.set('A1', 'ativou');");
        app.eventos_vba[0].aba = "Dados".into();
        let outra = app.workbook.criar_aba("Dados");
        app.bloquear_macros_do_arquivo();
        assert!(app.aviso_macros);
        app.update(Msg::AvisoMacrosFechar);
        app.update(Msg::Aba(outra));
        assert_eq!(app.workbook.aba(outra).unwrap().sheet.get_raw(0, 0), "");
        app.update(Msg::Aba(0));
        app.update(Msg::AtivarMacros);
        app.update(Msg::Aba(outra));
        assert_eq!(app.workbook.aba(outra).unwrap().sheet.get_raw(0, 0), "ativou");
    }
}
