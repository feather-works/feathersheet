//! Eventos convertidos do VBA rodando sozinhos, como no Excel:
//! `Worksheet_Change` (aoMudar) depois de cada mudança feita pela pessoa na
//! aba, com o endereço mudado em `valores.intervalo` (o `Target`), e
//! `Worksheet_Activate` (aoAtivar) ao trocar para a aba e `Worksheet_SelectionChange`
//! (aoSelecionar) quando a seleção muda na aba, com o endereço novo em
//! `valores.intervalo` (no arrasto, só ao soltar). Da pasta (aba vazia): `aoAbrir`
//! (o `Workbook_Open`) roda quando a pessoa clica em "Ativar macros" no arquivo recém-aberto,
//! seguido dos módulos "Executar ao abrir"; `antesDeSalvar` (o `Workbook_BeforeSave`) roda
//! antes do Salvar/Salvar como, e `wb.cancelSave()` não deixa gravar.
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
    /// A mensagem é um passo do arrasto da seleção.
    arrasto: bool,
}

impl App {
    pub(super) fn antes_dos_eventos(&self, arrasto: bool) -> Option<Antes> {
        if self.eventos_vba.is_empty() { return None; }
        let aba = self.workbook.aba(self.folha);
        Some(Antes { folha: self.folha, aba_id: aba.map(|a| a.id), versao: aba.map(|a| a.versao), arrasto })
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
        // Seleção nova na mesma aba (trocar de aba não conta, como no Excel).
        let agora = (self.folha, self.linha, self.coluna, self.fim_selecao);
        if !antes.arrasto && agora != self.selecao_evento {
            let mesma_aba = agora.0 == self.selecao_evento.0 && self.folha == antes.folha;
            self.selecao_evento = agora;
            if mesma_aba {
                if let Some(aba) = self.workbook.aba(self.folha).map(|a| a.sheet.name.clone()) {
                    let (r1, c1) = self.fim_selecao;
                    let alvo = endereco_area(&Rect::new(self.linha, self.coluna, r1, c1));
                    self.rodar_evento(&aba, "aoSelecionar", Some(&alvo));
                    // A macro pode ter selecionado outra coisa: não dispara de novo por isso.
                    self.selecao_evento = (self.folha, self.linha, self.coluna, self.fim_selecao);
                }
            }
        }
        self.macro_rodou = false;
    }

    /// `None`: não rodou (sem evento, macros bloqueadas) ou espera a resposta de uma pergunta.
    fn rodar_evento(&mut self, aba: &str, evento: &str, alvo: Option<&str>) -> Option<feathersheet_macros::Resultado> {
        self.rodar_evento_com(aba, evento, alvo, "")
    }

    /// `antes`: JavaScript que vai antes do código (os `valores` do evento).
    fn rodar_evento_com(&mut self, aba: &str, evento: &str, alvo: Option<&str>, antes: &str) -> Option<feathersheet_macros::Resultado> {
        if self.macros_bloqueadas { return None; }
        let e = self.eventos_vba.iter()
            .find(|e| e.evento == evento && e.aba.eq_ignore_ascii_case(aba)).cloned()?;
        let m = self.macro_catalogo.obter(&e.modulo)?;
        let mut codigo = format!("{antes}{}", m.code);
        if let Some(alvo) = alvo {
            codigo = format!("valores.intervalo={};\n{codigo}",
                serde_json::to_string(alvo).unwrap_or_default());
        }
        // Nos testes não: o diário é global e outro teste do console o confere.
        if !cfg!(test) {
            feathersheet_core::diario::anotar(trf!("[evento] {evento} da aba {aba}{}",
                alvo.map(|a| trf!(" em {a}")).unwrap_or_default()));
        }
        let r = self.rodar_macro(historico_macros::GatilhoMacro::Evento {
            evento: evento.to_owned(), aba: aba.to_owned(), intervalo: alvo.map(str::to_owned),
        }, &e.modulo, &codigo, Vec::new())?;
        if let Some(erro) = &r.erro {
            self.status = if aba.is_empty() { trf!("Evento {evento} da pasta: {erro}. Escritas revertidas.") }
                else { trf!("Evento {evento} da aba {aba}: {erro}. Escritas revertidas.") };
        }
        Some(r)
    }

    /// "Ativar macros" no arquivo recém-aberto: o `aoAbrir` da pasta e depois os módulos
    /// "Executar ao abrir", como o Workbook_Open e o Auto_Open do Excel. Uma vez por arquivo.
    pub(super) fn rodar_ao_abrir(&mut self) {
        if !std::mem::take(&mut self.ao_abrir_pendente) || self.macros_bloqueadas { return; }
        let _ = self.rodar_evento("", "aoAbrir", None);
        let automaticos: Vec<(String, String)> = self.macro_catalogo.macros.iter()
            .filter(|m| m.auto).map(|m| (m.name.clone(), m.code.clone())).collect();
        for (nome, codigo) in automaticos {
            // Uma pergunta Sim/Não deixa a macro esperando: as outras não passam na frente dela.
            if self.macro_pergunta.is_some() { break; }
            let gatilho = historico_macros::GatilhoMacro::Evento { evento: "aoAbrir".into(), aba: String::new(), intervalo: None };
            if let Some(feathersheet_macros::Resultado { erro: Some(erro), .. }) = self.rodar_macro(gatilho, &nome, &codigo, Vec::new()) {
                self.status = trf!("Erro ao executar {nome}: {erro}. Escritas revertidas.");
            }
        }
    }

    /// Antes do Salvar (`como`: Salvar como): roda o `antesDeSalvar` da pasta. `false` = não
    /// grava agora, porque a macro chamou `wb.cancelSave()` ou espera uma resposta (aí o
    /// Salvar volta sozinho depois dela, por [`Self::salvar_depois_do_evento`]).
    pub(super) fn antes_de_salvar(&mut self, como: bool) -> bool {
        if std::mem::take(&mut self.salvar_sem_evento) { return true; }
        let antes = format!("valores.comoSalvar={como};\n");
        match self.rodar_evento_com("", "antesDeSalvar", None, &antes) {
            Some(r) if r.cancelar => {
                self.status = tr!("A macro Antes de salvar cancelou a gravação.").into();
                false
            }
            Some(_) => true,
            None if self.macro_pergunta.is_some() => {
                self.salvar_apos_pergunta = Some(como);
                false
            }
            None => true,
        }
    }

    /// Antes de fechar a pasta (sair, Novo, abrir outro): roda o `antesDeFechar`. `false` = não fecha
    /// agora (`wb.cancelClose()`, ou espera uma resposta: aí fecha sozinho depois dela).
    pub(super) fn antes_de_fechar(&mut self, alvo: Pendente) -> bool {
        if std::mem::take(&mut self.fechar_sem_evento) { return true; }
        match self.rodar_evento("", "antesDeFechar", None) {
            Some(r) if r.cancelar => {
                self.status = tr!("A macro Antes de fechar não deixou fechar a pasta.").into();
                false
            }
            Some(_) => true,
            None if self.macro_pergunta.is_some() => {
                self.fechar_apos_pergunta = Some(alvo);
                false
            }
            None => true,
        }
    }

    /// Depois da resposta à pergunta da macro: se era o `antesDeSalvar` e ele não cancelou, salva.
    /// O `wb.cancelSave()` da continuação já desliga o pedido (`continuar_macro`).
    pub(super) fn salvar_depois_do_evento(&mut self) -> Task<Msg> {
        if self.macro_pergunta.is_some() { return Task::none(); }
        if let Some(alvo) = self.fechar_apos_pergunta.take() {
            self.fechar_sem_evento = true;
            return self.proteger(alvo);
        }
        let Some(como) = self.salvar_apos_pergunta.take() else { return Task::none() };
        self.salvar_sem_evento = true;
        self.update(if como { Msg::SalvarComo } else { Msg::Salvar })
    }
}

impl App {
    /// Arquivo aberto do disco com macros (módulos, eventos de aba, botões ou
    /// formas com macro): ficam bloqueadas até a pessoa ativar, como no Excel.
    pub(super) fn bloquear_macros_do_arquivo(&mut self) {
        let abas = || (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i));
        let botoes = abas().any(|a| a.sheet.bt.as_deref().is_some_and(|t| !matches!(t.trim(), "" | "[]")));
        // Uma forma transparente sobre a grade rodaria a macro no primeiro clique (#49).
        let formas = abas().any(|a| feathersheet_io::objetos_aba::ler_da_aba(&a.sheet)
            .objetos.iter().any(|o| o.macro_nome.is_some()));
        self.macros_bloqueadas = botoes || formas || !self.eventos_vba.is_empty()
            || !self.macro_catalogo.macros.is_empty();
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
        let mut riscos = feathersheet_macros::riscos::Riscos::default();
        for m in &self.macro_catalogo.macros { riscos.ler(&m.code); }
        let paragrafo = |t: String| text(t).size(12.5).line_height(1.4);
        let aviso = |titulo: String, t: String| pelicano::column![
            text(titulo).size(12.5).font(pelicano::Font { weight: 600, ..Default::default() }),
            paragrafo(t),
        ].spacing(2);
        let mut corpo = pelicano::column![paragrafo(tr!("Este arquivo tem macros. Elas podem alterar a planilha e rodar sozinhas (ao mudar células, ao trocar de aba ou ao clicar em botões). Ative só se você confia na origem do arquivo.").into())].spacing(10);
        let lista = |v: &[&str]| v.join(", ");
        if !riscos.grava.is_empty() {
            corpo = corpo.push(aviso(tr!("Grava arquivos no seu computador").into(),
                trf!("Usa {}: cria PDF, .xlsx ou .fshz na pasta desta planilha, e pode substituir um arquivo com o mesmo nome que já esteja lá.", lista(&riscos.grava))));
        }
        if !riscos.executa.is_empty() {
            corpo = corpo.push(aviso(tr!("Pede para rodar programas no seu computador").into(),
                tr!("Usa app.shell (o Shell do VBA). Antes de rodar, o FeatherSheet mostra o comando e pergunta.").into()));
        }
        if !riscos.binario.is_empty() {
            let vermelho = COR_ERRO();
            corpo = corpo.push(pelicano::column![
                text(tr!("O código da macro contém um binário")).size(13)
                    .font(pelicano::Font { weight: 700, ..Default::default() }).color(vermelho),
                text(riscos.binario.iter().map(|s| format!("• {s}")).collect::<Vec<_>>().join("\n"))
                    .size(12.5).line_height(1.4).color(vermelho),
                paragrafo(tr!("O FeatherSheet não procura vírus: só avisa que há um arquivo binário guardado como texto no código. Planilha comum não precisa disso; mantenha as macros desativadas se não sabe para que ele serve.").into()),
            ].spacing(4));
        }
        if !riscos.vba_fora.is_empty() {
            corpo = corpo.push(aviso(tr!("O VBA original rodava coisas fora do FeatherSheet").into(),
                trf!("A macro do Excel chamava {} (programas, internet ou o Windows). Essa parte não foi convertida e não roda aqui, mas é sinal para desconfiar do arquivo.", lista(&riscos.vba_fora))));
        }
        if !riscos.vba_apaga.is_empty() {
            corpo = corpo.push(aviso(tr!("O VBA original apagava arquivos do seu computador").into(),
                trf!("A macro do Excel usava {}. Essa parte não foi convertida e não roda aqui, mas é sinal para desconfiar do arquivo.", lista(&riscos.vba_apaga))));
        }
        self.dialogo(tr!("Ativar macros?"), corpo)
            .icone("alert-triangle")
            .largura(if riscos.vazio() { 480.0 } else { 540.0 })
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
    fn selecionar_roda_o_evento_e_o_arrasto_so_ao_soltar() {
        // Conta as execuções em Z1 e guarda o último Target em Z2.
        let mut app = app_com_evento("aoSelecionar",
            "sheet.set('Z1', Number(sheet.range('Z1').value || 0) + 1); sheet.set('Z2', valores.intervalo);");
        let valor = |app: &App, c: u32| app.workbook.aba(0).unwrap().sheet.get_raw(c, 25);
        app.update(Msg::Selecionar(2, 1));
        assert_eq!((valor(&app, 0).as_str(), valor(&app, 1).as_str()), ("1", "B3"));
        app.update(Msg::Selecionar(2, 1));
        assert_eq!(valor(&app, 0), "1", "a mesma seleção não dispara de novo");
        // Arrastar de B3 até D5: nada durante o arrasto, uma vez ao soltar.
        app.update(Msg::Estender(3, 2));
        app.update(Msg::Estender(4, 3));
        assert_eq!(valor(&app, 0), "1");
        app.update(Msg::AplicarPincel);
        assert_eq!((valor(&app, 0).as_str(), valor(&app, 1).as_str()), ("2", "B3:D5"));
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

    /// SEG-5: a macro gravada no `.fshz` volta bloqueada ao reabrir (o caminho de abrir do app,
    /// `instalar`), e o mesmo vale para um `.xlsm`; sem macro, nada é perguntado.
    #[test]
    fn reabrir_fshz_e_xlsm_com_macro_pede_para_ativar() {
        let pasta = std::env::temp_dir().join(format!("fs_seg5_{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let mut app = App::default();
        let mut cat = feather_macros::Catalogo::default();
        cat.macros.push(feather_macros::Macro { name: "Apagar".into(), auto: false, code: "sheet.clear('A1:Z100');".into() });
        assert!(app.persistir_modulos(cat, "ok"));
        let arquivo = pasta.join("com_macro.fshz");
        feathersheet_io::da_workbook::pasta_gravar_arquivo(&app.workbook, 0, &arquivo).unwrap();
        let abrir = |caminho: &std::path::Path| {
            let lida = feathersheet_io::fshz_read::ler_fshz_arquivo_compacto(&arquivo).unwrap();
            let mut novo = App::default();
            novo.instalar(caminho.to_path_buf(), feathersheet_io::para_workbook::para_workbook_soltando(lida), 0);
            novo
        };
        for caminho in [arquivo.clone(), pasta.join("com_macro.xlsm")] {
            let mut novo = abrir(&caminho);
            assert!(novo.macros_bloqueadas && novo.aviso_macros, "{}", caminho.display());
            assert!(!novo.macros_liberadas(), "rodar antes de ativar é recusado");
        }
        // Sem macro nenhuma: abre sem perguntar.
        let mut vazio = App::default();
        vazio.instalar(pasta.join("sem.fshz"), feathersheet_core::planilha::workbook::Workbook::new(), 0);
        assert!(!vazio.aviso_macros);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn modulo_solto_tambem_pede_para_ativar_e_o_aviso_lista_os_riscos() {
        let mut app = App::default();
        app.macro_catalogo = Default::default();
        app.macro_catalogo.macros.push(feather_macros::Macro { name: "Relatorio".into(), auto: false,
            code: "sheet.exportPdf('r.pdf');\n// VBA não convertido (API):\n//   Kill \"C:\\x\"".into() });
        app.bloquear_macros_do_arquivo();
        assert!(app.macros_bloqueadas && app.aviso_macros);
        let _ = app.sobrepor_aviso_macros(text("").into());
        app.update(Msg::AvisoMacrosFechar);
        app.macro_visual.documento = "modulo:Relatorio".into();
        app.executar_macro();
        assert!(app.macro_saida.contains("desativadas"), "{}", app.macro_saida);
        assert!(app.aviso_macros, "rodar à mão reabre o aviso");
    }

    fn app_com_evento_da_pasta(evento: &str, codigo: &str) -> App {
        let mut app = App::default();
        app.macro_catalogo = Default::default();
        let mut c = feather_macros::Catalogo::default();
        c.macros.push(feather_macros::Macro {
            name: crate::app::macros::organizacao::nome_evento("", evento), code: codigo.into(), auto: false });
        assert!(app.persistir_modulos(c, "ok"));
        assert_eq!(app.eventos_vba.len(), 1, "o módulo Eventos/<evento> liga o evento da pasta");
        assert_eq!(app.eventos_vba[0].aba, "");
        app
    }

    /// O Workbook_Open (aoAbrir) roda no "Ativar macros" do arquivo recém-aberto, uma vez,
    /// e os módulos "Executar ao abrir" rodam depois dele; antes de ativar, nada.
    #[test]
    fn ao_abrir_roda_ao_ativar_macros_uma_vez_e_depois_os_automaticos() {
        let mut app = app_com_evento_da_pasta("aoAbrir",
            "sheet.set('A1', (Number(sheet.get('A1')) || 0) + 1);");
        let mut c = app.macro_catalogo.clone();
        c.macros.push(feather_macros::Macro { name: "Auto".into(), auto: true, code: "sheet.set('B1', sheet.get('A1'));".into() });
        assert!(app.persistir_modulos(c, "ok"));
        app.bloquear_macros_do_arquivo();
        app.ao_abrir_pendente = app.macros_bloqueadas;
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Empty, "bloqueadas: não roda");
        app.update(Msg::AtivarMacros);
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Number(1.0));
        assert_eq!(app.workbook.valor(0, 0, 1), feathersheet_core::Value::Number(1.0), "o automático roda depois do aoAbrir");
        app.macros_bloqueadas = true;
        app.update(Msg::AtivarMacros);
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Number(1.0), "uma vez por arquivo aberto");
    }

    /// O antesDeSalvar (Workbook_BeforeSave) roda antes de gravar; `wb.cancelSave()` impede.
    #[test]
    fn antes_de_salvar_pode_cancelar_a_gravacao() {
        let pasta = std::env::temp_dir().join(format!("fs_antes_salvar_{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let mut app = app_com_evento_da_pasta("antesDeSalvar",
            "if (!sheet.get('B1')) wb.cancelSave(); else sheet.set('C1', valores.comoSalvar ? 'como' : 'ctrl+s');");
        app.caminho = Some(pasta.join("a.fshz"));
        let _ = app.update(Msg::Salvar);
        assert!(!app.salvando, "cancelado: não grava");
        assert!(app.status.contains("cancelou"), "{}", app.status);
        app.workbook.escrever_celula(0, 0, 1, "ok".into());
        let _ = app.update(Msg::Salvar);
        assert!(app.salvando, "sem cancelar: grava");
        assert_eq!(app.workbook.valor(0, 0, 2), feathersheet_core::Value::Text("ctrl+s".into()));
        app.concluir_gravacao_teste(pasta.join("a.fshz"), false);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// A pergunta (MsgBox vbYesNo) no antesDeSalvar: o Salvar espera a resposta e volta sozinho
    /// no "Sim"; no "Não" a macro cancela e nada é gravado.
    #[test]
    fn antes_de_salvar_com_pergunta_salva_depois_da_resposta() {
        let pasta = std::env::temp_dir().join(format!("fs_antes_salvar_p_{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let mut app = app_com_evento_da_pasta("antesDeSalvar", "if (!confirm('Gravar?')) wb.cancelSave();");
        app.caminho = Some(pasta.join("b.fshz"));
        let _ = app.update(Msg::Salvar);
        assert!(app.macro_pergunta.is_some() && !app.salvando);
        let _ = app.update(Msg::MacroResponder(false));
        assert!(!app.salvando, "Não: cancelado");
        let _ = app.update(Msg::Salvar);
        let _ = app.update(Msg::MacroResponder(true));
        assert!(app.salvando, "Sim: o Salvar volta sozinho");
        assert!(!app.salvar_sem_evento, "a marca é gasta");
        app.concluir_gravacao_teste(pasta.join("b.fshz"), false);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// O antesDeFechar (Workbook_BeforeClose) roda antes do "salvar alterações?"; `wb.cancelClose()` impede
    /// fechar (aqui, criar uma pasta nova); com pergunta, fecha sozinho depois do "Sim".
    #[test]
    fn antes_de_fechar_pode_impedir_e_espera_a_resposta() {
        let mut app = app_com_evento_da_pasta("antesDeFechar", "if (sheet.get('A1') === 'travar') wb.cancelClose();");
        app.workbook.escrever_celula(0, 0, 0, "travar".into());
        app.alterado = false;
        let _ = app.update(Msg::Novo);
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Text("travar".into()), "não fechou");
        assert!(app.status.contains("não deixou"), "{}", app.status);
        // Com pergunta: "Não" mantém a pasta; "Sim" fecha.
        let mut app = app_com_evento_da_pasta("antesDeFechar", "if (!confirm('Fechar?')) wb.cancelClose();");
        app.workbook.escrever_celula(0, 0, 0, "x".into());
        app.alterado = false;
        let _ = app.update(Msg::Novo);
        assert!(app.macro_pergunta.is_some());
        let _ = app.update(Msg::MacroResponder(false));
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Text("x".into()));
        let _ = app.update(Msg::Novo);
        let _ = app.update(Msg::MacroResponder(true));
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Empty, "Sim: a pasta nova entrou");
        assert!(!app.fechar_sem_evento, "a marca é gasta");
    }
}

