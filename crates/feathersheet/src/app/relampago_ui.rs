//! Preenchimento relâmpago (Ctrl+E; Página Inicial > Preencher ▾ e Dados >
//! Ferramentas de dados). Sem diálogo, como no Excel: a coluna da célula
//! ativa é preenchida a partir dos exemplos já digitados nela, e a barra de
//! status diz quantas linhas entraram. A síntese mora no motor
//! (`feathersheet_core::operacoes::relampago`).
use super::*;
use feathersheet_core::operacoes::relampago;

impl App {
    pub(super) fn preencher_relampago(&mut self) {
        self.edicao_menu = None;
        if !self.confirmar() { return; }
        let (folha, linha, coluna) = (self.folha, self.linha, self.coluna);
        let inicio = std::time::Instant::now();
        let plano = {
            let Some(aba) = self.workbook.aba(folha) else { return };
            let ler = |r: u32, c: u32| relampago::texto_exibido(&aba.sheet, r, c);
            relampago::planejar(&ler, linha, coluna)
        };
        let plano = match plano {
            Ok(p) => p,
            Err(e) => {
                self.status = e.mensagem().into();
                return;
            }
        };
        self.workbook.editar(folha, |sheet| relampago::aplicar_plano(sheet, coluna, &plano));
        self.alterado = true;
        let n = plano.escrever.len();
        let sem = if plano.sem_padrao > 0 {
            trf!("; {} linha(s) sem o padrão ficaram vazias", plano.sem_padrao)
        } else { String::new() };
        self.status = trf!(
            "Preenchimento relâmpago: {n} linha(s) preenchida(s) em {}:{} a partir de {} exemplo(s){sem}. Ctrl+Z desfaz.",
            cell_name(plano.escrever[0].0, coluna),
            cell_name(plano.escrever[n - 1].0, coluna),
            plano.exemplos.len());
        feathersheet_core::diario::anotar(trf!(
            "[relâmpago] {n} linha(s) · {} pedaço(s) · {:.1} ms",
            plano.programa.pedacos.len(), inicio.elapsed().as_secs_f64() * 1000.0));
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::values::Value;

    #[test]
    fn ctrl_e_preenche_a_coluna_e_um_desfazer_volta() {
        let mut app = App::default();
        for (i, nome) in ["João Silva", "Maria Souza", "Pedro Alves"].iter().enumerate() {
            app.workbook.escrever_celula(0, i as u32, 0, (*nome).into());
        }
        app.workbook.escrever_celula(0, 0, 1, "Silva".into());
        app.linha = 1;
        app.coluna = 1;
        app.fim_selecao = (1, 1);
        // Pelo teclado, como a pessoa faz: Ctrl+E na janela principal.
        let id = window::novo_id();
        app.janelas.principal = Some(id);
        let _ = app.update(Msg::Teclado(id, keyboard::Key::Character("e".into()),
            keyboard::Modifiers::CTRL));
        assert_eq!(app.workbook.valor(0, 1, 1), Value::Text("Souza".into()));
        assert_eq!(app.workbook.valor(0, 2, 1), Value::Text("Alves".into()));
        assert!(app.status.contains("2 linha(s) preenchida(s)"), "{}", app.status);
        let _ = app.update(Msg::Desfazer);
        assert!(app.workbook.valor(0, 1, 1).is_empty());
        assert!(app.workbook.valor(0, 2, 1).is_empty());
        assert_eq!(app.workbook.valor(0, 0, 1), Value::Text("Silva".into()));
        // Sem exemplo: avisa e não escreve.
        app.workbook.escrever_celula(0, 0, 1, String::new());
        let _ = app.update(Msg::PreenchimentoRelampago);
        assert!(app.status.contains("digite um exemplo"), "{}", app.status);
        assert!(app.workbook.valor(0, 1, 1).is_empty());
    }
}
