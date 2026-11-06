//! Execução das macros com a pasta em volta: arquivo e seleção para o
//! `wb.name`/`selection()`, a pergunta Sim/Não (`confirm`, `MsgBox` com
//! botões), as mensagens de `alert`/`MsgBox` e os arquivos pedidos
//! (`exportPdf`, `saveSheetsAs`: ver `arquivos.rs`).
//!
//! A pergunta é respondida pela pessoa numa janela do próprio app. Como a
//! macro não pausa no meio, ela termina respondendo "Não" sem gravar nada;
//! depois da resposta roda de novo do começo com as respostas já dadas.
use super::*;
use historico_macros::GatilhoMacro;

mod arquivos;
mod programas;
pub(super) use programas::ProgramasMacro;
pub(super) mod em_fundo;

pub(super) struct PerguntaMacro {
    nome: String,
    codigo: String,
    respostas: Vec<bool>,
    /// Botões já escolhidos nas perguntas de vários botões (`app.choose`, o `MsgBox` do VBA).
    escolhas: Vec<u32>,
    /// A pergunta pendente é de vários botões; `None` é Sim/Não.
    escolha: Option<feathersheet_macros::Escolha>,
    /// Textos já digitados nas perguntas de texto (`prompt`, o `InputBox`); `None` = Cancelar.
    textos: Vec<Option<String>>,
    /// A pergunta pendente pede um texto: o que está escrito no campo.
    entrada: Option<String>,
    texto: String,
    /// O que disparou a macro e a linha dela no histórico: a reexecução
    /// depois da resposta continua na mesma linha.
    gatilho: GatilhoMacro,
    historico: u64,
    ms: u128,
}

impl App {
    /// Roda a macro e aplica o que ela pediu ao app. `None` quando ficou
    /// esperando uma resposta Sim/Não. Todo caminho que roda macro passa por
    /// aqui (menos o MCP, ver `historico_macros.rs`) dizendo o que a disparou,
    /// e a execução entra no histórico de macros.
    pub(super) fn rodar_macro(&mut self, gatilho: GatilhoMacro, nome: &str, codigo: &str, respostas: Vec<bool>)
        -> Option<feathersheet_macros::Resultado>
    {
        self.rodar_macro_registrando(gatilho, nome, codigo, respostas, Vec::new(), Vec::new(), None)
    }

    /// `continua`: a linha do histórico e o tempo já gasto, quando é a
    /// reexecução depois de uma resposta Sim/Não.
    fn rodar_macro_registrando(&mut self, gatilho: GatilhoMacro, nome: &str, codigo: &str,
        respostas: Vec<bool>, escolhas: Vec<u32>, textos: Vec<Option<String>>, continua: Option<(u64, u128)>) -> Option<feathersheet_macros::Resultado>
    {
        let inicio_execucao = std::time::Instant::now();
        let contexto = feathersheet_macros::Contexto { textos: textos.clone(), ..self.contexto_da_macro(respostas.clone(), escolhas.clone()) };
        let r = feathersheet_macros::executar_com(&mut self.workbook, self.folha, codigo, contexto);
        self.concluir_macro(gatilho, nome, codigo, respostas, escolhas, textos, continua, inicio_execucao, r)
    }

    /// O arquivo, a seleção, as respostas Sim/Não e os campos do formulário para a macro.
    pub(super) fn contexto_da_macro(&self, respostas: Vec<bool>, escolhas: Vec<u32>) -> feathersheet_macros::Contexto {
        let (r1, c1) = self.fim_selecao;
        let selecao = endereco_area(&Rect::new(self.linha, self.coluna, r1, c1));
        feathersheet_macros::Contexto {
            arquivo: self.caminho.clone(), selecao: Some(selecao), respostas, escolhas,
            separador_decimal: Some(self.regionalizacao().decimal_separator),
            valores: self.macro_valores.clone(), ..Default::default()
        }
    }

    /// O que vem depois da execução (aqui ou na thread do `em_fundo`): histórico, pergunta
    /// Sim/Não, formulário, arquivos pedidos e mensagens.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn concluir_macro(&mut self, gatilho: GatilhoMacro, nome: &str, codigo: &str,
        respostas: Vec<bool>, escolhas: Vec<u32>, textos: Vec<Option<String>>, continua: Option<(u64, u128)>, inicio_execucao: std::time::Instant,
        mut r: feathersheet_macros::Resultado) -> Option<feathersheet_macros::Resultado>
    {
        use historico_macros::{Execucao, ResultadoExecucao};
        // O que a macro escreveu não dispara os eventos do VBA.
        self.macro_rodou = true;
        let (id_anterior, ms_anterior) = continua.unzip();
        let ms_anterior = ms_anterior.unwrap_or(0);
        // Uma linha por execução; a continuação do Sim/Não atualiza a mesma.
        let anotar = |app: &mut App, resultado: ResultadoExecucao, alterou: bool, arquivos: Vec<String>| {
            let ms = ms_anterior + inicio_execucao.elapsed().as_millis();
            let execucao = Execucao {
                id: 0, hora: String::new(), macro_: nome.to_owned(), gatilho: gatilho.clone(),
                resultado, ms, alterou, respostas: respostas.clone(), arquivos,
            };
            let id = match id_anterior {
                Some(id) => app.historico_macros.atualizar(id, |e| {
                    let hora = std::mem::take(&mut e.hora);
                    *e = Execucao { id: e.id, hora, ..execucao };
                }),
                None => app.historico_macros.registrar(execucao),
            };
            (id, ms)
        };
        if let Some(texto) = r.pergunta.take() {
            let (historico, ms) = anotar(self, ResultadoExecucao::Esperando(texto.clone()), false, Vec::new());
            self.macro_pergunta = Some(PerguntaMacro {
                nome: nome.to_owned(), codigo: codigo.to_owned(), respostas, escolhas, escolha: r.escolha.take(),
                textos, entrada: r.entrada.take(), texto,
                gatilho, historico, ms,
            });
            return None;
        }
        self.macro_valores = None;
        self.alterado |= r.alterou;
        // Formulário: fechar (`hideComponent`), abrir (`showComponent`) e mudar campos (`field`), nessa ordem.
        if r.fechar_formulario { self.formulario_macro = None; }
        if let Some((nome_form, dados)) = r.formulario.take() { self.abrir_formulario_macro(&nome_form, &dados); }
        self.aplicar_campos_formulario(std::mem::take(&mut r.campos));
        // Os arquivos pedidos (PDF, abas num arquivo novo) só com a macro inteira bem.
        // `showGridlines`: a mesma marca por aba das abas do Excel sem grade.
        for &(id, ligada) in if r.erro.is_none() { r.grades.as_slice() } else { &[] } {
            if ligada { self.grade_oculta.remove(&id); } else { self.grade_oculta.insert(id); }
        }
        if r.erro.is_none() && !r.links.is_empty() {
            let falhas = self.abrir_links_da_macro(&r.links);
            r.mensagens.extend(falhas);
        }
        if r.erro.is_none() && !r.formas.is_empty() {
            let falhas = objetos::aplicar_formas_da_macro(&mut self.workbook, &r.formas);
            self.alterado = true;
            r.mensagens.extend(falhas);
        }
        if r.erro.is_none() && !r.imagens.is_empty() {
            let falhas = objetos::aplicar_imagens_da_macro(&mut self.workbook, &r.imagens, &self.grade_oculta);
            self.alterado = true;
            r.mensagens.extend(falhas);
        }
        if r.erro.is_none() && !r.filtros.is_empty() {
            let falhas = tabelas::aplicar_filtros_da_macro(&mut self.workbook, &r.filtros);
            self.alterado = true;
            r.mensagens.extend(falhas);
        }
        if r.erro.is_none() && !r.protecoes.is_empty() {
            let falhas = self.aplicar_protecoes_da_macro(&r.protecoes);
            r.mensagens.extend(falhas);
        }
        let mut arquivos = Vec::new();
        if r.erro.is_none() && !r.arquivos.is_empty() {
            let (gravados, falhas) = self.gravar_arquivos_da_macro(&r.arquivos);
            arquivos.extend(gravados.iter().map(|l| l.trim_start_matches("[macro] ").to_owned()));
            arquivos.extend(falhas.iter().cloned());
            r.saida.extend(gravados);
            r.mensagens.extend(falhas);
        }
        let resultado = match &r.erro {
            Some(e) => ResultadoExecucao::Erro(e.clone()),
            // Respondeu Não e a macro saiu sem mudar nada: desistiu.
            None if respostas.last() == Some(&false) && !r.alterou && arquivos.is_empty() =>
                ResultadoExecucao::Cancelada,
            None => ResultadoExecucao::Ok,
        };
        anotar(self, resultado, r.alterou, arquivos);
        // `app.shell`: nada roda sem a pessoa ver o comando e dizer Sim.
        if r.erro.is_none() && !r.programas.is_empty() {
            self.pedir_programas_da_macro(nome, codigo, std::mem::take(&mut r.programas));
        }
        if !r.mensagens.is_empty() {
            self.macro_mensagens = std::mem::take(&mut r.mensagens);
        }
        if let Some(aba) = r.aba_ativa.filter(|&a| a != self.folha) {
            let _ = self.update(Msg::Aba(aba));
        }
        Some(r)
    }

    pub(super) fn responder_macro(&mut self, sim: bool) {
        // Pergunta de texto (`InputBox`): OK manda o que está no campo; Cancelar, nada.
        if self.macro_pergunta.as_ref().is_some_and(|p| p.entrada.is_some()) {
            let Some(mut p) = self.macro_pergunta.take() else { return };
            let escrito = p.entrada.take();
            p.textos.push(escrito.filter(|_| sim));
            return self.continuar_macro(p);
        }
        // Pergunta de vários botões respondida por Sim/Não (Enter/Esc, histórico): o padrão ou o último (Cancelar/Não).
        if let Some(e) = self.macro_pergunta.as_ref().and_then(|p| p.escolha.as_ref()) {
            let i = if sim { e.padrao } else { e.botoes.len() - 1 };
            return self.escolher_na_macro(i);
        }
        let Some(mut p) = self.macro_pergunta.take() else { return };
        p.respostas.push(sim);
        self.continuar_macro(p);
    }

    /// O botão `i` da pergunta de vários botões (`app.choose`, `MsgBox` com Abortar/Repetir/Ignorar…).
    pub(super) fn escolher_na_macro(&mut self, i: usize) {
        let Some(mut p) = self.macro_pergunta.take() else { return };
        let Some(e) = p.escolha.take() else { return };
        p.escolhas.push(i.min(e.botoes.len() - 1) as u32);
        self.continuar_macro(p);
    }

    /// O campo da pergunta de texto mudou.
    pub(super) fn digitar_na_macro(&mut self, t: String) {
        if let Some(e) = self.macro_pergunta.as_mut().and_then(|p| p.entrada.as_mut()) { *e = t; }
    }

    fn continuar_macro(&mut self, p: PerguntaMacro) {
        let Some(r) = self.rodar_macro_registrando(p.gatilho, &p.nome, &p.codigo, p.respostas, p.escolhas, p.textos,
            Some((p.historico, p.ms))) else { return };
        self.macro_saida = r.saida.join("\n");
        self.status = match r.erro {
            Some(e) => trf!("Erro ao executar {}: {e}. Escritas revertidas.", p.nome),
            None => r.status.unwrap_or_else(|| trf!("Macro {} concluída.", p.nome)),
        };
        self.trocar_selecao(self.linha, self.coluna);
    }

    /// Janela da pergunta ou das mensagens, sobre a janela inteira.
    /// Pergunta (Sim/Não) ou mensagem da macro: diálogo modal sem fechar
    /// por fora nem pelo x, porque a macro espera a resposta.
    pub(super) fn sobrepor_dialogo_macro<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        use super::dialogo_interno::dlg;
        if self.macro_pergunta.is_none() {
            if let Some(p) = &self.macro_programas { return self.dialogo_programas_da_macro(p, base); }
        }
        let (titulo, icone, corpo) = if let Some(p) = &self.macro_pergunta {
            let icone = match p.escolha.as_ref().map(|e| e.icone) {
                Some(16 | 48) => "alert-triangle",
                Some(64) => "info-circle",
                _ => "help",
            };
            (p.nome.clone(), icone, p.texto.as_str())
        } else if let Some(texto) = self.macro_mensagens.first() {
            (tr!("Mensagem da macro").to_string(), "info-circle", texto.as_str())
        } else {
            return base;
        };
        let entrada = self.macro_pergunta.as_ref().and_then(|p| p.entrada.as_deref());
        let conteudo: Element<'a, Msg> = match entrada {
            Some(valor) => pelicano::column![
                text(corpo).size(13).line_height(1.45),
                text_input("", valor).on_input(Msg::MacroDigitar).on_submit(Msg::MacroResponder(true)).size(13).padding([6, 8]),
            ].spacing(10).into(),
            None => text(corpo).size(13).line_height(1.45).into(),
        };
        let d = self.dialogo(titulo, conteudo)
            .icone(icone)
            .largura(460.0)
            .clique_fora_fecha(false);
        let escolha = self.macro_pergunta.as_ref().and_then(|p| p.escolha.as_ref());
        let d = if let Some(e) = escolha {
            // Os botões do MsgBox na ordem do Excel; o padrão é o primário (Enter).
            let rotulo = |b: u32| match b {
                1 => "OK".to_string(), 2 => tr!("Cancelar").to_string(), 3 => tr!("Anular").to_string(),
                4 => tr!("Repetir").to_string(), 5 => tr!("Ignorar").to_string(), 6 => tr!("Sim").to_string(), _ => tr!("Não").to_string(),
            };
            e.botoes.iter().enumerate().fold(d, |d, (i, &b)| if i == e.padrao {
                d.primario(rotulo(b), Some(Msg::MacroEscolher(i)))
            } else {
                d.botao(dlg::botao_secundario(rotulo(b), Some(Msg::MacroEscolher(i))))
            })
        } else if entrada.is_some() {
            d.botao(dlg::botao_secundario(tr!("Cancelar"), Some(Msg::MacroResponder(false))))
                .primario("OK", Some(Msg::MacroResponder(true)))
        } else if self.macro_pergunta.is_some() {
            d.botao(dlg::botao_secundario(tr!("Não"), Some(Msg::MacroResponder(false))))
                .primario(tr!("Sim"), Some(Msg::MacroResponder(true)))
        } else {
            d.primario("OK", Some(Msg::MacroMensagemOk))
        };
        d.sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn pergunta_espera_a_resposta_e_roda_de_novo() {
        let mut app = App::default();
        let codigo = "alert('oi'); if (confirm('Gravar?')) sheet.set('A1', 5);";
        assert!(app.rodar_macro(GatilhoMacro::manual("teste"), "M", codigo, vec![]).is_none());
        assert_eq!(app.macro_pergunta.as_ref().unwrap().texto, "Gravar?");
        assert!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0).is_empty());
        app.update(Msg::MacroResponder(true));
        assert!(app.macro_pergunta.is_none());
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Number(5.));
        assert_eq!(app.macro_mensagens, vec!["oi".to_string()]);
        app.update(Msg::MacroMensagemOk);
        assert!(app.macro_mensagens.is_empty());
    }

    #[test]
    fn pergunta_de_tres_botoes_devolve_o_botao_escolhido() {
        let mut app = App::default();
        let codigo = "sheet.set('A1', app.choose('Arquivo em uso', [3, 4, 5], 16, 1));";
        assert!(app.rodar_macro(GatilhoMacro::manual("teste"), "M", codigo, vec![]).is_none());
        let e = app.macro_pergunta.as_ref().unwrap().escolha.clone().unwrap();
        assert_eq!((e.botoes, e.icone, e.padrao), (vec![3, 4, 5], 16, 1));
        app.update(Msg::MacroEscolher(2));
        assert!(app.macro_pergunta.is_none());
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Number(2.));
        // Enter (Sim) escolhe o padrão; Esc (Não), o último.
        assert!(app.rodar_macro(GatilhoMacro::manual("teste"), "M", codigo, vec![]).is_none());
        app.update(Msg::MacroResponder(true));
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Number(1.));
    }

    #[test]
    fn pergunta_de_texto_manda_o_que_foi_digitado() {
        let mut app = App::default();
        let codigo = "const n = prompt('Quantos?', '3'); sheet.set('A1', n === null ? 'cancelou' : Number(n) * 2);";
        assert!(app.rodar_macro(GatilhoMacro::manual("teste"), "M", codigo, vec![]).is_none());
        app.update(Msg::MacroDigitar("21".into()));
        app.update(Msg::MacroResponder(true));
        assert!(app.macro_pergunta.is_none());
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Number(42.));
        assert!(app.rodar_macro(GatilhoMacro::manual("teste"), "M", codigo, vec![]).is_none());
        app.update(Msg::MacroResponder(false));
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Text("cancelou".into()));
    }

    #[test]
    fn macro_ativa_outra_aba() {
        let mut app = App::default();
        app.workbook.inserir_aba("Dados", 1);
        app.rodar_macro(GatilhoMacro::manual("teste"), "M", "wb.activate('Dados');", vec![]).unwrap();
        assert_eq!(app.folha, 1);
    }
}
