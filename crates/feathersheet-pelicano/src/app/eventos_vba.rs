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
}
