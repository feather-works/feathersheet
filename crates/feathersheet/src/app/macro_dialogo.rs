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

pub(super) struct PerguntaMacro {
    nome: String,
    codigo: String,
    respostas: Vec<bool>,
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
        self.rodar_macro_registrando(gatilho, nome, codigo, respostas, None)
    }

    /// `continua`: a linha do histórico e o tempo já gasto, quando é a
    /// reexecução depois de uma resposta Sim/Não.
    fn rodar_macro_registrando(&mut self, gatilho: GatilhoMacro, nome: &str, codigo: &str,
        respostas: Vec<bool>, continua: Option<(u64, u128)>) -> Option<feathersheet_macros::Resultado>
    {
        use historico_macros::{Execucao, ResultadoExecucao};
        let inicio_execucao = std::time::Instant::now();
        let (r1, c1) = self.fim_selecao;
        let selecao = endereco_area(&Rect::new(self.linha, self.coluna, r1, c1));
        let contexto = feathersheet_macros::Contexto {
            arquivo: self.caminho.clone(), selecao: Some(selecao), respostas: respostas.clone(),
            valores: self.macro_valores.clone(),
        };
        let mut r = feathersheet_macros::executar_com(&mut self.workbook, self.folha, codigo, contexto);
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
                nome: nome.to_owned(), codigo: codigo.to_owned(), respostas, texto,
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
        if !r.mensagens.is_empty() {
            self.macro_mensagens = std::mem::take(&mut r.mensagens);
        }
        if let Some(aba) = r.aba_ativa.filter(|&a| a != self.folha) {
            let _ = self.update(Msg::Aba(aba));
        }
        Some(r)
    }

    pub(super) fn responder_macro(&mut self, sim: bool) {
        let Some(mut p) = self.macro_pergunta.take() else { return };
        p.respostas.push(sim);
        let Some(r) = self.rodar_macro_registrando(p.gatilho, &p.nome, &p.codigo, p.respostas,
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
        let (titulo, icone, corpo) = if let Some(p) = &self.macro_pergunta {
            (p.nome.clone(), "help", p.texto.as_str())
        } else if let Some(texto) = self.macro_mensagens.first() {
            (tr!("Mensagem da macro").to_string(), "info-circle", texto.as_str())
        } else {
            return base;
        };
        let d = self.dialogo(titulo, text(corpo).size(13).line_height(1.45))
            .icone(icone)
            .largura(460.0)
            .clique_fora_fecha(false);
        let d = if self.macro_pergunta.is_some() {
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
    fn macro_ativa_outra_aba() {
        let mut app = App::default();
        app.workbook.inserir_aba("Dados", 1);
        app.rodar_macro(GatilhoMacro::manual("teste"), "M", "wb.activate('Dados');", vec![]).unwrap();
        assert_eq!(app.folha, 1);
    }
}
