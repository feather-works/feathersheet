//! Métricas da seleção no rodapé, portadas de ui/barras/statusbar.ts.
//! Usa o armazenamento esparso do motor e cache por aba/versão/intervalo.
//! Áreas grandes são somadas em blocos durante pulsos curtos, sem bloquear
//! cada redesenho ou cada movimento de seleção. O aviso de erro da célula
//! ativa (botão vermelho à direita, como no TS) também fica em cache.
use super::*;
use feathersheet_core::formula::explicar::explicar_erro;
use feathersheet_core::planilha::sheet::ContagemSelecao;

/// Medido (examples/medir_contagem.rs do core): 200 mil números em 0,8 ms;
/// no .parquet de 13,7 milhões (examples/medir_contagem_parquet.rs do io),
/// 5 ns por número e 14 ns por texto. Até 2 milhões de células a conta sai
/// direto (~10–30 ms); acima, em lotes grandes: lotes de 4.096 linhas
/// custavam 10× mais que a conta inteira (34 s contra 4,3 s na tabela).
const CELULAS_DIRETAS: u64 = 2_000_000;
const CELULAS_POR_LOTE: u64 = 500_000;
/// Com fórmula nas colunas da seleção, cada célula pode custar microssegundos
/// (um `MÍNIMOSES` em 13,7 milhões de linhas): lotes pequenos, para o
/// orçamento por quadro valer, e nada de contar tudo de uma vez. Eram 500
/// mil fórmulas por lote, ~1 s cada, e a janela "não está respondendo"
/// (17/10/2026).
const CELULAS_POR_LOTE_COM_FORMULA: u64 = 16_384;
const CELULAS_DIRETAS_COM_FORMULA: u64 = 16_384;

/// Alguma coluna de `c0..=c1` tem fórmula empacotada ou avulsa.
fn tem_formula(sheet: &feathersheet_core::planilha::sheet::Sheet, c0: u32, c1: u32) -> bool {
    sheet.formulas.colunas().iter().any(|&c| c >= c0 && c <= c1)
        // Muitas avulsas: não vale varrê-las a cada quadro, trata como fórmula.
        || sheet.cells.len() > 100_000
        || sheet.cells.keys().any(|&k| { let c = feathersheet_core::celulas::refs::key_col(k); c >= c0 && c <= c1 })
}
const ORCAMENTO_MS: u128 = 12;
const COR_ERRO: pelicano::Color = pelicano::Color::rgb(220.0 / 255.0, 75.0 / 255.0, 77.0 / 255.0);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Chave {
    folha: usize,
    versao: u32,
    area: Rect,
    /// Toda pasta recém-aberta começa na versão 1: sem estes dois, abrir
    /// outro arquivo com a mesma seleção reaproveitava a conta do anterior
    /// (ou emendava os lotes dos dois) e mostrava como terminada.
    usado: Rect,
    celulas: usize,
    /// As outras áreas do Ctrl+clique.
    extras: Vec<Rect>,
}
/// Folha, versão, linha e coluna da célula cujo erro está em cache.
type ChaveErro = (usize, u32, u32, u32);

#[derive(Default)]
pub(super) struct Estado {
    chave: Option<Chave>,
    pub(super) pendente: bool,
    /// A seleção cai fora da área usada: o rodapé fica em branco, como no TS.
    fora_da_area: bool,
    conta: ContagemSelecao,
    primeira: u32,
    proxima: u32,
    ultima: u32,
    erro_chave: Option<ChaveErro>,
    erro: Option<String>,
}
impl Estado {
    fn porcentagem(&self) -> u32 {
        let total = (self.ultima as u64).saturating_sub(self.primeira as u64) + 1;
        let pronto = (self.proxima as u64).saturating_sub(self.primeira as u64);
        ((pronto as f64 * 100.0 / total as f64).round() as u32).min(100)
    }
}

/// Células da seleção (a área inteira, mesmo fora da parte usada).
fn celulas_da_area(area: &Rect) -> u64 {
    (area.r1 as u64 - area.r0 as u64 + 1) * (area.c1 as u64 - area.c0 as u64 + 1)
}

impl App {
    /// Sincroniza a cache pelo estado atual sem recalcular durante repaint.
    pub(super) fn sincronizar_resumo(&self) {
        // O conversor de 1M linhas trabalha em múltiplos ticks. Recontar a
        // seleção original após cada lote seria trabalho quadrático inútil.
        // A versão publicada após commit/cancel reativa o resumo normalmente.
        if self.assistente_divisao.as_ref()
            .is_some_and(|e| e.conversao_em_andamento())
        {
            let mut resumo = self.resumo_selecao.borrow_mut();
            resumo.pendente = false;
            resumo.chave = None; // ao terminar, refaz o resumo de verdade
            return;
        }
        let area = self.area_selecionada();
        let versao = self.workbook.versao(self.folha);
        let (usado, celulas) = self.workbook.aba(self.folha)
            .map(|a| (a.sheet.used_rect(), a.sheet.len()))
            .unwrap_or((Rect::new(0, 0, 0, 0), 0));
        let chave = Chave { folha: self.folha, versao, area: area.clone(), usado, celulas, extras: self.selecoes_extras.clone() };
        let mut estado = self.resumo_selecao.borrow_mut();
        if estado.chave.as_ref() == Some(&chave) {
            return;
        }
        estado.chave = Some(chave);
        estado.conta = ContagemSelecao::default();
        estado.pendente = false;
        estado.fora_da_area = false;
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        if aba.sheet.is_empty() {
            return;
        }
        let usado = aba.sheet.used_rect();
        let recorte = |a: &Rect| {
            let (r0, c0, r1, c1) = (a.r0.max(usado.r0), a.c0.max(usado.c0), a.r1.min(usado.r1), a.c1.min(usado.c1));
            (r1 >= r0 && c1 >= c0).then(|| Rect::new(r0, c0, r1, c1))
        };
        // As áreas do Ctrl+clique somam junto, como no Excel (a que se repete conta duas vezes).
        // shortcut: as extras contam direto, sem lotes; uma coluna inteira de milhões trava o quadro.
        let mut extras = false;
        for a in self.selecoes_extras.iter().filter_map(recorte) {
            let parte = aba.sheet.contar_selecao(&a, Some(&self.workbook));
            estado.conta.somar_lote(&parte);
            extras = true;
        }
        let Some(Rect { r0, c0, r1, c1, .. }) = recorte(&area) else {
            estado.fora_da_area = !extras;
            return;
        };
        let recortada = Rect::new(r0, c0, r1, c1);
        // A chave corresponde à seleção original; o processo usa o recorte.
        let celulas = (r1 as u64 - r0 as u64 + 1) * (c1 as u64 - c0 as u64 + 1);
        let diretas = if tem_formula(&aba.sheet, c0, c1) { CELULAS_DIRETAS_COM_FORMULA } else { CELULAS_DIRETAS };
        if celulas <= diretas {
            let parte = aba.sheet.contar_selecao(&recortada, Some(&self.workbook));
            estado.conta.somar_lote(&parte);
        } else {
            // Redefine a área processada, mantendo a chave da seleção original.
            estado.primeira = r0;
            estado.proxima = r0;
            estado.ultima = r1;
            estado.pendente = true;
        }
    }

    /// Erro da célula ativa, com cache por célula e versão: só recalcula quando
    /// a célula ou o conteúdo mudam, nunca a cada quadro.
    fn sincronizar_erro(&self) {
        let versao = self.workbook.versao(self.folha);
        let chave: ChaveErro = (self.folha, versao, self.linha, self.coluna);
        let mut estado = self.resumo_selecao.borrow_mut();
        if estado.erro_chave == Some(chave) {
            return;
        }
        estado.erro_chave = Some(chave);
        estado.erro = explicar_erro(&self.workbook, self.folha, self.linha, self.coluna)
            .map(|e| format!("{} — {}", e.code, e.summary));
    }

    /// Cada pulso termina dentro do orçamento esperado em casos normais.
    /// Os números parciais aparecem só com o aviso "Somando N%…" na frente.
    pub(super) fn avancar_resumo(&self) {
        self.sincronizar_resumo();
        let mut estado = self.resumo_selecao.borrow_mut();
        if !estado.pendente {
            return;
        }
        let Some(chave) = estado.chave.as_ref() else { return };
        let Some(aba) = self.workbook.aba(chave.folha) else {
            estado.pendente = false;
            return;
        };
        let usado = aba.sheet.used_rect();
        let c0 = chave.area.c0.max(usado.c0);
        let c1 = chave.area.c1.min(usado.c1);
        if c1 < c0 {
            estado.pendente = false;
            return;
        }
        let colunas = c1 as u64 - c0 as u64 + 1;
        let por_lote = if tem_formula(&aba.sheet, c0, c1) { CELULAS_POR_LOTE_COM_FORMULA } else { CELULAS_POR_LOTE };
        let linhas = (por_lote / colunas).clamp(1, 500_000) as u32;
        let inicio = Instant::now();
        while estado.pendente {
            let r0 = estado.proxima;
            let r1 = r0.saturating_add(linhas - 1).min(estado.ultima);
            let parte = aba.sheet.contar_selecao(&Rect::new(r0, c0, r1, c1),
                Some(&self.workbook));
            estado.conta.somar_lote(&parte);
            if r1 >= estado.ultima {
                estado.pendente = false;
                break;
            }
            estado.proxima = r1 + 1;
            if inicio.elapsed().as_millis() >= ORCAMENTO_MS {
                break;
            }
        }
    }

    /// As contas da seleção atual e o número de células da área (para o
    /// menu Personalizar barra de status mostrar os valores ao lado).
    pub(super) fn contas_da_selecao(&self) -> (ContagemSelecao, u64) {
        self.sincronizar_resumo();
        let estado = self.resumo_selecao.borrow();
        let celulas = estado.chave.as_ref().map_or(0, |c| celulas_da_area(&c.area) + c.extras.iter().map(celulas_da_area).sum::<u64>());
        (estado.conta.clone(), celulas)
    }

    /// Resumo curto na barra existente de 36 px, à esquerda do zoom, com o
    /// aviso de erro da célula ativa à direita (clicar abre a explicação).
    pub(super) fn view_resumo_selecao(&self) -> Element<'_, Msg> {
        if self.assistente_divisao.as_ref()
            .is_some_and(|e| e.conversao_em_andamento())
        {
            return container(text(tr!("Convertendo texto em colunas…")).size(11)
                .color(COR_TEXTO_MUTED()))
                .padding([0, 8]).into();
        }
        self.sincronizar_resumo();
        self.sincronizar_erro();
        let largura = self.tamanho_principal.width;
        let estado = self.resumo_selecao.borrow();
        let celulas = estado.chave.as_ref().map_or(0, |c| celulas_da_area(&c.area) + c.extras.iter().map(celulas_da_area).sum::<u64>());
        let contas = barra_status::texto_contas(&estado.conta, &self.config.barra_status, celulas, largura);
        let texto = if estado.fora_da_area {
            // Fora da área usada não há o que contar; só as vazias, se pedidas.
            if self.barra_status_mostra("vazias") {
                trf!("Vazias: {}", barra_status::formatar_inteiro(celulas))
            } else { String::new() }
        } else if estado.pendente {
            trf!("Somando {}%…   {}", estado.porcentagem(), contas)
        } else {
            contas
        };
        let resumo = text(texto).size(11).color(COR_TEXTO_MUTED())
            .no_wrap();
        let mut linha = row![resumo].spacing(8).align(pelicano::Align::Center);
        if let Some(erro) = estado.erro.as_ref().filter(|_| self.barra_status_mostra("aviso_erro")) {
            let aviso = button(text(erro.clone()).size(11).color(COR_ERRO)
                .no_wrap())
                .padding([0, 4])
                .style(|_, _| pelicano::Style::default())
                .on_press(Msg::ExplicacaoErroAbrir);
            linha = linha.push(
                container(aviso).max_width(largura * 0.52).clip(true)
            );
        }
        // Sem folga à direita: as contas encostam no "−" do zoom, como no Excel.
        container(linha).padding(pelicano::Padding { left: 8.0, ..pelicano::Padding::ZERO }).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn coluna_de_formulas_pesadas_soma_em_fatias_curtas() {
        let mut app = App::default();
        let n = 300_000u32;
        app.workbook.editar(0, |s| {
            for r in 0..n {
                s.set_cell(r, 0, if r % 3 == 0 { "a" } else { "b" });
                s.set_cell(r, 1, &r.to_string());
            }
            s.set_cell(0, 2, "=MÍNIMOSES(B:B;A:A;A1)");
            assert!(s.preencher_formula_faixa_vazia(2, 1, n - 1, "=MÍNIMOSES(B:B;A:A;A2)"));
        });
        app.linha = 0; app.coluna = 2; app.fim_selecao = (n - 1, 2);
        // O primeiro quadro monta o índice do MÍNIMOSES; os outros só leem.
        app.avancar_resumo();
        let mut pior = std::time::Duration::ZERO;
        for _ in 0..20 {
            let t = std::time::Instant::now();
            app.avancar_resumo();
            pior = pior.max(t.elapsed());
        }
        assert!(pior < std::time::Duration::from_millis(150), "quadro de {pior:?}");
        while app.resumo_selecao.borrow().pendente { app.avancar_resumo(); }
        assert_eq!(app.resumo_selecao.borrow().conta.numbers, n as u64);
    }

    #[test]
    fn status_altera_com_selecao_e_edicao_mas_nao_com_redesenho() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "10".into());
        app.workbook.escrever_celula(0, 1, 0, "20".into());
        app.workbook.escrever_celula(0, 1, 1, "teste".into());
        app.linha = 0; app.coluna = 0; app.fim_selecao = (1, 1);
        let _: Element<'_, Msg> = app.view_resumo_selecao();
        assert_eq!(app.resumo_selecao.borrow().conta.sum, 30.);
        assert_eq!(app.resumo_selecao.borrow().conta.count, 3);
        let _ = app.view_resumo_selecao();
        app.workbook.escrever_celula(0, 0, 0, "40".into());
        let _ = app.view_resumo_selecao();
        assert_eq!(app.resumo_selecao.borrow().conta.sum, 60.);
        app.linha = 1; app.coluna = 1; app.fim_selecao = (1, 1);
        let _ = app.view_resumo_selecao();
        assert_eq!(app.resumo_selecao.borrow().conta.count, 1);
        assert_eq!(app.resumo_selecao.borrow().conta.numbers, 0);
    }
    #[test]
    fn areas_do_ctrl_clique_somam_junto() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "10".into());
        app.workbook.escrever_celula(0, 0, 1, "99".into());
        app.workbook.escrever_celula(0, 0, 2, "5".into());
        // A1 e C1, pulando B1.
        app.selecoes_extras = vec![Rect::new(0, 0, 0, 0)];
        app.linha = 0; app.coluna = 2; app.fim_selecao = (0, 2);
        let _ = app.view_resumo_selecao();
        let conta = app.resumo_selecao.borrow().conta.clone();
        assert_eq!((conta.sum, conta.numbers, conta.min, conta.max), (15., 2, 5., 10.));
    }
    #[test]
    fn selecao_fora_da_area_usada_fica_em_branco() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "10".into());
        app.linha = 5; app.coluna = 5; app.fim_selecao = (5, 5);
        let _ = app.view_resumo_selecao();
        assert!(app.resumo_selecao.borrow().fora_da_area);
        app.linha = 0; app.coluna = 0; app.fim_selecao = (0, 0);
        let _ = app.view_resumo_selecao();
        assert!(!app.resumo_selecao.borrow().fora_da_area);
    }
    #[test]
    fn resumo_fica_suspenso_enquanto_converte_grande_selecao() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "A;B".into());
        app.workbook.escrever_celula(0, 3_000_000, 0, "C;D".into());
        app.linha = 0; app.coluna = 0; app.fim_selecao = (3_000_000, 0);
        app.sincronizar_resumo();
        assert!(app.resumo_selecao.borrow().pendente);
        app.abrir_assistente_divisao();
        app.evento_assistente_divisao(assistente_divisao::Evento::Concluir);
        assert!(app.assistente_divisao.as_ref().unwrap().conversao_em_andamento());
        let _: Element<'_, Msg> = app.view_resumo_selecao();
        app.sincronizar_resumo();
        assert!(!app.resumo_selecao.borrow().pendente);
        app.evento_assistente_divisao(assistente_divisao::Evento::Cancelar);
        let _: Element<'_, Msg> = app.view_resumo_selecao();
        assert!(app.resumo_selecao.borrow().pendente);
    }

    #[test]
    fn selecao_grande_processa_em_lotes_sem_ficar_preso_no_limite() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "10".into());
        app.workbook.escrever_celula(0, 3_000_000, 0, "30".into());
        app.linha = 0; app.coluna = 0; app.fim_selecao = (MAX_ROWS - 1, 0);
        let _ = app.view_resumo_selecao();
        assert!(app.resumo_selecao.borrow().pendente);
        // Simula os pulsos até terminar, sem aguardar relógio de interface.
        for _ in 0..100 {
            if !app.resumo_selecao.borrow().pendente { break; }
            app.avancar_resumo();
        }
        let estado = app.resumo_selecao.borrow();
        assert!(!estado.pendente);
        assert_eq!(estado.conta.count, 2);
        assert_eq!(estado.conta.sum, 40.);
    }
}
