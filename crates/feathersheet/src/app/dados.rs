//! Adaptação das operações existentes no motor para eventos e controles Iced.
use super::*;
use pelicano::checkbox; use pelicano::column; use pelicano::pick_list; use pelicano::tooltip; use pelicano::TooltipPosition;
use feathersheet_core::operacoes::ops;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResultadoBusca {
    pub aba: usize,
    pub linha: u32,
    pub coluna: u32,
}

#[derive(Default)]
pub(super) struct Busca {
    pub aberta: bool,
    pub texto: String,
    pub substituicao: String,
    pub opcoes: ops::SearchOptions,
    /// A busca pode atravessar todas as abas visíveis, sem importar o TS.
    pub pasta_inteira: bool,
    pub resultados: Vec<ResultadoBusca>,
    pub total: usize,
    pub mostrar_todos: bool,
    /// Aba "Substituir" da janela (Ctrl+H); `false` é "Localizar" (Ctrl+F).
    pub substituir: bool,
    /// "Opções" expandidas, como o botão "Opções >>" do Excel.
    pub mais_opcoes: bool,
    /// Pesquisar por colunas: percorre A1, A2… antes de B1.
    pub por_colunas: bool,
    /// Quanto a janela foi arrastada a partir do canto superior direito
    /// (x positivo = para a esquerda). Fica guardado entre aberturas.
    pub deslocamento: crate::compat::Vector,
    /// Arrastando pela barra de título: `Some(None)` até o primeiro
    /// movimento, depois a última posição do cursor.
    pub arrasto: Option<Option<pelicano::Point>>,
}

/// Campo "Localizar" recebe o foco ao abrir a janela.
pub(super) const ID_CAMPO_BUSCA: &str = "feathersheet.busca-localizar";

/// Opções das listas da janela, na ordem do Excel.
const DENTRO: [&str; 2] = ["Planilha", "Pasta de trabalho"];
const PESQUISAR: [&str; 2] = ["Por linhas", "Por colunas"];
const EXAMINAR: [&str; 2] = ["Fórmulas", "Valores"];

/// Corta o texto para caber na coluna da lista, sem quebrar linha.
fn cortar(texto: &str, maximo: usize) -> String {
    let linha = texto.replace(['\n', '\r'], " ");
    if linha.chars().count() <= maximo { return linha; }
    let mut curto: String = linha.chars().take(maximo.saturating_sub(1)).collect();
    curto.push('…');
    curto
}

impl App {
    /// Define as abas da consulta sem navegar por guias ocultas. O TS
    /// pesquisava na pasta; na Iced só as abas visíveis podem receber foco.
    fn abas_da_busca(&self) -> Vec<usize> {
        if self.busca.pasta_inteira {
            self.workbook.abas_visiveis().collect()
        } else { vec![self.folha] }
    }

    /// Reaproveita o mesmo motor Rust de busca indexada/colunar.
    fn achar_na_aba(&self, indice: usize) -> Vec<ops::SearchMatch> {
        let Some(aba) = self.workbook.aba(indice) else { return Vec::new() };
        let mut achadas = ops::find_all_com(&aba.sheet, &self.busca.texto, self.busca.opcoes,
            |r, c| self.workbook.valor(indice, r, c));
        if self.busca.por_colunas { achadas.sort_unstable_by_key(|m| (m.col, m.row)); }
        achadas
    }

    /// Ordem de visita da busca: linha a linha ou coluna a coluna.
    fn ordem_busca(&self, linha: u32, coluna: u32) -> (u32, u32) {
        if self.busca.por_colunas { (coluna, linha) } else { (linha, coluna) }
    }

    /// Abre a janela na aba pedida e põe o cursor no campo "Localizar".
    pub(super) fn abrir_busca(&mut self, substituir: bool) -> Task<Msg> {
        self.busca.aberta = true;
        self.busca.substituir = substituir;
        self.contexto = None;
        Task::focus(ID_CAMPO_BUSCA)
    }

    /// Navega para a próxima/anterior, com volta circular entre abas.
    /// Cada resultado temporário pertence a uma aba por vez: não copia
    /// simultaneamente as correspondências da pasta inteira.
    pub(super) fn localizar_direcao(&mut self, reverso: bool) -> Task<Msg> {
        if !self.confirmar() { return Task::none(); }
        if self.busca.texto.is_empty() {
            self.status = tr!("Digite o conteúdo que deseja localizar.").into();
            return Task::none();
        }
        let indices = self.abas_da_busca();
        let Some(posicao) = indices.iter().position(|&i| i == self.folha) else {
            return Task::none();
        };
        let total = indices.len();
        let pos_atual = self.ordem_busca(self.linha, self.coluna);
        let mut destino = None;
        for distancia in 0..total {
            let p = if reverso { (posicao + total - distancia) % total }
                else { (posicao + distancia) % total };
            let i = indices[p];
            let achadas = self.achar_na_aba(i);
            let proxima = if distancia == 0 {
                if reverso {
                    achadas.iter().rev().find(|m| self.ordem_busca(m.row, m.col) < pos_atual)
                } else {
                    achadas.iter().find(|m| self.ordem_busca(m.row, m.col) > pos_atual)
                }
            } else if reverso { achadas.last() } else { achadas.first() };
            if let Some(m) = proxima {
                destino = Some(ResultadoBusca { aba: i, linha: m.row, coluna: m.col });
                break;
            }
        }
        // Quando nenhuma outra aba tem achados, voltar ao início da ativa.
        if destino.is_none() {
            let achadas = self.achar_na_aba(self.folha);
            let m = if reverso { achadas.last() } else { achadas.first() };
            destino = m.map(|m| ResultadoBusca {
                aba: self.folha, linha: m.row, coluna: m.col,
            });
        }
        let Some(destino) = destino else {
            self.status = tr!("Nenhuma ocorrência encontrada.").into();
            return Task::none();
        };
        self.ir_para_resultado(destino)
    }

    /// Navega ao índice da lista "Localizar todos" sem alterar a consulta.
    pub(super) fn ir_para_resultado(&mut self, destino: ResultadoBusca) -> Task<Msg> {
        if self.workbook.aba(destino.aba).is_none_or(|a| a.sheet.oculta) {
            self.status = tr!("A aba encontrada não está disponível.").into();
            return Task::none();
        }
        let tarefa = if destino.aba != self.folha {
            self.update(Msg::Aba(destino.aba))
        } else { Task::none() };
        if self.folha == destino.aba {
            self.trocar_selecao(destino.linha, destino.coluna);
            self.status = trf!("Encontrado: {}!{}",
                self.workbook.nomes()[destino.aba],
                cell_name(destino.linha, destino.coluna));
        }
        tarefa
    }

    /// Mostra até 200 endereços, sem montar uma UI com milhões de entradas.
    pub(super) fn localizar_todos(&mut self) {
        if !self.confirmar() { return; }
        self.busca.resultados.clear();
        self.busca.total = 0;
        self.busca.mostrar_todos = true;
        if self.busca.texto.is_empty() {
            self.status = tr!("Digite o conteúdo que deseja localizar.").into();
            return;
        }
        const LIMITE_LISTA: usize = 200;
        for i in self.abas_da_busca() {
            let achadas = self.achar_na_aba(i);
            self.busca.total = self.busca.total.saturating_add(achadas.len());
            let disponiveis = LIMITE_LISTA.saturating_sub(self.busca.resultados.len());
            self.busca.resultados.extend(achadas.iter().take(disponiveis)
                .map(|m| ResultadoBusca { aba: i, linha: m.row, coluna: m.col }));
        }
        self.status = if self.busca.total > LIMITE_LISTA {
            trf!("{} ocorrência(s); a lista mostra as primeiras {LIMITE_LISTA}.", self.busca.total)
        } else { trf!("{} ocorrência(s) encontrada(s).", self.busca.total) };
    }

    /// Substitui a correspondência da célula ativa; se não bater, não altera.
    /// A escrita/undo continua no motor Rust, em uma única transação.
    pub(super) fn substituir_atual(&mut self) -> Task<Msg> {
        if !self.confirmar() { return Task::none(); }
        if self.busca.texto.is_empty() {
            self.status = tr!("Digite o conteúdo que deseja substituir.").into();
            return Task::none();
        }
        let (row, col) = (self.linha, self.coluna);
        let Some(aba) = self.workbook.aba(self.folha) else { return Task::none() };
        let bruto = aba.sheet.get_raw(row, col);
        let comparado = if self.busca.opcoes.in_formulas {
            bruto.clone()
        } else {
            feathersheet_core::celulas::values::to_text(
                &self.workbook.valor(self.folha, row, col))
        };
        if !ops::is_hit(&comparado, &self.busca.texto, self.busca.opcoes) {
            self.status = tr!("A célula ativa não corresponde ao termo pesquisado.").into();
            return Task::none();
        }
        if !self.busca.opcoes.whole_cell && !ops::is_hit(&bruto, &self.busca.texto,
            ops::SearchOptions { in_formulas: true, ..self.busca.opcoes })
        {
            self.status = tr!("Correspondência somente no valor calculado. Use Célula inteira para substituí-la.").into();
            return Task::none();
        }
        let mut alteradas = 0;
        self.workbook.editar(self.folha, |sheet| {
            alteradas = ops::replace_all_em(sheet, &[ops::SearchMatch { row, col }],
                &self.busca.texto, &self.busca.substituicao, self.busca.opcoes);
        });
        if alteradas > 0 {
            self.alterado = true;
            self.busca.mostrar_todos = false;
            self.busca.resultados.clear();
            self.trocar_selecao(row, col);
            self.status = trf!("Substituído: {}.", cell_name(row, col));
            self.localizar_direcao(false)
        } else {
            self.status = tr!("Nenhuma alteração na célula atual.").into();
            Task::none()
        }
    }

    /// Substituir tudo da versão TS, na aba ou em todas as abas visíveis.
    /// Histórico permanece independente por aba, como no Workbook existente.
    pub(super) fn substituir_tudo(&mut self) {
        if !self.confirmar() { return; }
        if self.busca.texto.is_empty() {
            self.status = tr!("Digite o conteúdo que deseja substituir.").into();
            return;
        }
        let mut quantidade = 0usize;
        let mut abas_afetadas = 0usize;
        for i in self.abas_da_busca() {
            let achadas = self.achar_na_aba(i);
            if achadas.is_empty() { continue; }
            let mut alteradas = 0;
            self.workbook.editar(i, |sheet| {
                alteradas = ops::replace_all_em(sheet, &achadas, &self.busca.texto,
                    &self.busca.substituicao, self.busca.opcoes);
            });
            if alteradas > 0 {
                quantidade += alteradas;
                abas_afetadas += 1;
            }
        }
        if quantidade > 0 { self.alterado = true; }
        self.busca.mostrar_todos = false;
        self.busca.resultados.clear();
        self.busca.total = 0;
        self.trocar_selecao(self.linha, self.coluna);
        self.status = if abas_afetadas > 1 {
            trf!("{quantidade} célula(s) substituída(s) em {abas_afetadas} abas (desfazer por aba).")
        } else {
            trf!("{quantidade} célula(s) substituída(s).")
        };
    }

    pub(super) fn ordenar(&mut self, crescente: bool) {
        self.confirmar();
        let area = self.area_selecionada();
        let Some(aba) = self.workbook.aba(self.folha) else { return; };
        let plano = ops::planejar_ordenacao(area.clone(), &[ops::SortKey { col:self.coluna, ascending:crescente }],
            self.cabecalho_dados, aba.sheet.used_rect().r1, |r,c| self.workbook.valor(self.folha,r,c));
        if let Some(plano) = plano {
            self.workbook.editar(self.folha, |sheet| ops::aplicar_ordenacao(sheet,area.c0,area.c1,&plano));
            self.alterado = true;
            self.edicao = self.workbook.aba(self.folha).unwrap().sheet.get_raw(self.linha,self.coluna);
            self.status = tr!("Seleção ordenada pela coluna da célula ativa").into();
        } else { self.status = tr!("Seleção já ordenada ou sem linhas suficientes").into(); }
    }

    /// Processamento streaming, sem carregar a seleção inteira na memória.
    const LOTE_DIVISAO: u32 = 2048;

    /// Divisão rápida da Ribbon: valida TODOS os destinos antes de escrever.
    /// Não recalcula valores nem aloca strings em linhas sem separador,
    /// não regrava células não divididas e preserva um único Ctrl+Z.
    pub(super) fn dividir_texto(&mut self, separador: &str) {
        use feathersheet_core::celulas::values::to_text;

        self.confirmar();
        if separador.is_empty() {
            self.status = tr!("Informe o separador antes de dividir o texto").into();
            return;
        }
        let area = self.area_selecionada();
        if area.c0 != area.c1 {
            self.status = tr!("Selecione apenas uma coluna para dividir o texto").into();
            return;
        }
        let Some(aba) = self.workbook.aba(self.folha) else { return; };
        let ultima_linha = area.r1.min(aba.sheet.used_rect().r1);
        if area.r0 > ultima_linha {
            self.status = tr!("A seleção não contém texto para dividir").into();
            return;
        }

        // 1. Validação completa, inclusive dos lotes posteriores.
        // Mantém apenas o resultado das fórmulas que efetivamente dividem,
        // pois recalculá-las durante as escritas poderia mudar os resultados.
        let mut formulas_originais = std::collections::HashMap::<u32, String>::new();
        let mut quantidade = 0usize;
        for linha in area.r0..=ultima_linha {
            let original = aba.sheet.get_raw(linha, area.c0);
            let formula = original.starts_with('=');
            if !formula && !original.contains(separador) { continue; }

            let texto = to_text(&self.workbook.valor(self.folha, linha, area.c0));
            if !texto.contains(separador) { continue; }
            let partes = texto.split(separador).count();
            if partes <= 1 { continue; }
            if (area.c0 as usize).saturating_add(partes) > MAX_COLS as usize {
                self.status = tr!("A divisão ultrapassaria a última coluna da planilha").into();
                return;
            }
            for deslocamento in 1..partes {
                let coluna = area.c0 + deslocamento as u32;
                if !aba.sheet.get_raw(linha, coluna).is_empty() {
                    self.status = trf!(
                        "Divisão cancelada: {} já contém dados", cell_name(linha, coluna)
                    );
                    return;
                }
            }
            if formula { formulas_originais.insert(linha, texto); }
            quantidade += 1;
        }
        if quantidade == 0 {
            self.status = trf!("Nenhum texto contém o separador {separador:?}");
            return;
        }

        let opcoes = ops::OpcoesDivisao {
            delimitadores: vec![separador.to_owned()],
            ..Default::default()
        };
        // 2. Escrever apenas linhas que contêm o separador em lotes de 2048.
        // A transação externa agrupa os lotes para um único desfazer.
        let mut alteradas = 0usize;
        self.workbook.editar(self.folha, |sheet| {
            sheet.abrir_transacao();
            let mut inicio = area.r0;
            while inicio <= ultima_linha {
                let fim = inicio.saturating_add(Self::LOTE_DIVISAO - 1).min(ultima_linha);
                let mut linhas = Vec::new();
                for linha in inicio..=fim {
                    let original = sheet.get_raw(linha, area.c0);
                    let texto = if original.starts_with('=') {
                        // Só fórmulas aprovadas na primeira passagem entram.
                        let Some(capturado) = formulas_originais.get(&linha) else { continue; };
                        capturado.clone()
                    } else {
                        if !original.contains(separador) { continue; }
                        to_text(&sheet.get_value(linha, area.c0))
                    };
                    if !texto.contains(separador) { continue; }
                    let partes = ops::dividir_com_opcoes(&texto, &opcoes);
                    if partes.len() > 1 { linhas.push((linha, partes)); }
                }
                if !linhas.is_empty() {
                    alteradas += ops::aplicar_divisao(sheet, area.c0, &linhas, None);
                    // Também limitar o histórico do comando rápido da Ribbon.
                    sheet.compactar_lote_divisao();
                }
                inicio = fim + 1;
            }
            sheet.fechar_transacao();
        });
        if alteradas > 0 {
            self.alterado = true;
            self.edicao = self.workbook.aba(self.folha)
                .map(|aba| aba.sheet.get_raw(self.linha, self.coluna))
                .unwrap_or_default();
        }
        self.status = trf!(
            "{alteradas} linha(s) dividida(s) pelo separador {separador:?}"
        );
    }

    pub(super) fn aparar_espacos(&mut self) {
        self.confirmar();
        let area = self.area_selecionada();
        let mut quantidade = 0;
        self.workbook.editar(self.folha, |sheet| quantidade = ops::trim_spaces(sheet,area));
        if quantidade > 0 { self.alterado = true; }
        self.edicao = self.workbook.aba(self.folha).unwrap().sheet.get_raw(self.linha,self.coluna);
        self.status = trf!("Espaços aparados em {quantidade} célula(s)");
    }

    /// AutoSoma (e Média, Contagem, Máximo, Mínimo do menu dele): grava a
    /// fórmula na célula já confirmada, como se tivesse dado Enter. Antes só
    /// punha o texto na barra fx e esperava o Enter (o `confirmar` não grava
    /// com o editor fechado).
    pub(super) fn autosoma(&mut self, funcao: &str) {
        if !self.confirmar() { return; }
        let Some(formula) = ops::formula_da_autosoma(self.linha,self.coluna,
            |r,c| self.workbook.valor(self.folha,r,c)) else {
            self.status = tr!("Não há números acima ou à esquerda para somar").into();
            return;
        };
        let formula = formula.replacen("=SOMA(", &format!("={funcao}("), 1);
        let (folha, linha, coluna) = (self.folha, self.linha, self.coluna);
        self.workbook.escrever_celula(folha, linha, coluna, formula);
        self.edicao = self.workbook.aba(folha).map(|a| a.sheet.get_raw(linha, coluna)).unwrap_or_default();
        self.alterado = true;
        self.status = trf!("{} atualizado", cell_name(linha, coluna));
        self.atualizar_circulos();
    }

    /// Janela "Localizar e substituir" no modelo do Excel: flutua sobre a
    /// grade (não empurra o layout), abas Localizar/Substituir, rótulos à
    /// esquerda, "Opções" recolhíveis e os botões na última linha.
    pub(super) fn view_busca(&self) -> Option<Element<'_, Msg>> {
        if !self.busca.aberta { return None; }
        let b = &self.busca;
        let negrito = pelicano::Font { weight: 600, ..Default::default() };
        const ROTULO: f32 = 104.0;
        let rotulo = |t: &'static str| text(t).size(12.5).width(Length::Fixed(ROTULO));
        let botao = |t: &'static str, msg: Msg| button(text(t).size(12.5))
            .padding([6, 12]).style(estilo_botao_secundario).on_press(msg);
        let icone_dica = |icone: &'static str, dica: &'static str, msg: Msg| tooltip(
            button(container(feather_ui::icones::icone(icone, 15.0))
                    .width(Length::FILL).align_x(pelicano::Align::Center)
                    .height(Length::FILL).align_y(pelicano::Align::Center))
                .width(Length::Fixed(28.0)).height(Length::Fixed(28.0)).padding(0)
                .style(estilo_botao_pequeno(false)).on_press(msg),
            dica,
            TooltipPosition::Bottom,
        ).padding([4, 8]).style(estilo_dica);
        let aba = |t: &'static str, ativa: bool, substituir: bool| button(text(t).size(12.5))
            .padding([6, 14]).style(estilo_botao_pequeno(ativa))
            .on_press(Msg::BuscaAba(substituir));

        // A barra de título arrasta a janela, como no Excel.
        let titulo = row![
            pelicano::mouse_area(row![
                feather_ui::icones::icone("search", 17.0),
                text(tr!("Localizar e substituir")).size(15).font(negrito),
                Space::new(Length::FILL, 0.0),
            ].spacing(8).align(pelicano::Align::Center).height(Length::Fixed(28.0)))
                .on_press(Msg::BuscaArrastoInicio)
                .cursor_icon(crate::compat::mouse::Interaction::Grab),
            icone_dica("x", tr!("Fechar (Esc)"), Msg::FecharBusca),
        ].spacing(8).align(pelicano::Align::Center);
        let abas = row![
            aba(tr!("Localizar"), !b.substituir, false),
            aba(tr!("Substituir"), b.substituir, true),
        ].spacing(4);

        let mut campos = column![row![
            rotulo(tr!("Localizar:")),
            text_input("", &b.texto)
                .id(ID_CAMPO_BUSCA)
                .on_input(Msg::BuscaTexto).on_submit(Msg::Localizar)
                .size(13).padding([5, 8]).width(Length::FILL),
            icone_dica("chevron-up", tr!("Localizar anterior"), Msg::LocalizarAnterior),
        ].spacing(8).align(pelicano::Align::Center)].spacing(8);
        if b.substituir {
            campos = campos.push(row![
                rotulo(tr!("Substituir por:")),
                text_input("", &b.substituicao)
                    .on_input(Msg::Substituicao).on_submit(Msg::SubstituirAtual)
                    .size(13).padding([5, 8]).width(Length::FILL),
                Space::new(Length::Fixed(28.0), 0.0),
            ].spacing(8).align(pelicano::Align::Center));
        }

        let opcoes_botao = button(row![
            text(tr!("Opções")).size(12.5),
            feather_ui::icones::icone(if b.mais_opcoes { "chevron-up" } else { "chevron-down" }, 14.0),
        ].spacing(5).align(pelicano::Align::Center))
            .padding([5, 10]).style(estilo_botao_pequeno(b.mais_opcoes))
            .on_press(Msg::BuscaMaisOpcoes);
        let mut corpo = column![titulo, abas, campos].spacing(12);
        if b.mais_opcoes {
            let lista = |opcoes: &'static [&'static str], escolhida: &'static str, msg: fn(&'static str) -> Msg|
                pick_list(crate::idioma::lista(opcoes), Some(tr!(escolhida)), msg).size(12.5)
                    .width(Length::Fixed(170.0)).padding([4, 8]);
            let esquerda = column![
                row![rotulo(tr!("Dentro:")), lista(&DENTRO, DENTRO[b.pasta_inteira as usize],
                    |s| Msg::BuscaPasta(s == tr!(DENTRO[1])))].align(pelicano::Align::Center),
                row![rotulo(tr!("Pesquisar:")), lista(&PESQUISAR, PESQUISAR[b.por_colunas as usize],
                    |s| Msg::BuscaPorColunas(s == tr!(PESQUISAR[1])))].align(pelicano::Align::Center),
                row![rotulo(tr!("Examinar:")), lista(&EXAMINAR, EXAMINAR[!b.opcoes.in_formulas as usize],
                    |s| Msg::BuscaFormulas(s == tr!(EXAMINAR[0])))].align(pelicano::Align::Center),
            ].spacing(8);
            let direita = column![
                checkbox(tr!("Diferenciar maiúsculas"), b.opcoes.match_case)
                    .on_toggle(Msg::BuscaMaiusculas).size(12.5),
                checkbox(tr!("Célula inteira"), b.opcoes.whole_cell)
                    .on_toggle(Msg::BuscaInteira).size(12.5),
            ].spacing(10);
            corpo = corpo.push(row![esquerda, Space::new(Length::FILL, 0.0), direita]
                .spacing(12));
        }
        corpo = corpo.push(row![Space::new(Length::FILL, 0.0), opcoes_botao]);

        // Botões embaixo, como no Excel: os de substituir só na aba deles,
        // "Localizar próxima" é a ação principal.
        let mut botoes = row(Vec::new()).spacing(7).align(pelicano::Align::Center);
        if b.substituir {
            botoes = botoes
                .push(botao(tr!("Substituir tudo"), Msg::SubstituirTudo))
                .push(botao(tr!("Substituir"), Msg::SubstituirAtual));
        }
        botoes = botoes
            .push(Space::new(Length::FILL, 0.0))
            .push(botao(tr!("Localizar tudo"), Msg::LocalizarTodos))
            .push(button(text(tr!("Localizar próxima")).size(12.5)).padding([6, 12])
                .style(estilo_botao_primario).on_press(Msg::Localizar));
        // Um fechar só: o x da barra de título (e o Esc).
        corpo = corpo.push(botoes);

        if b.mostrar_todos { corpo = corpo.push(self.view_busca_resultados()); }

        let janela = container(corpo).padding([16, 20]).width(Length::Fixed(560.0))
            .style(|_: &pelicano::Theme| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL())),
                border: pelicano::Border { color: COR_BORDA(), width: 1.0, radius: [12.0; 4] },
                shadow: Some(pelicano::Shadow {
                    color: pelicano::Color { a: 0.28, ..pelicano::Color::BLACK },
                    offset: pelicano::Point::new(0.0, 6.0),
                    blur: 22.0,
                }),
                ..Default::default()
            });
        // Posição: canto superior direito da grade mais o arrasto.
        let d = b.deslocamento;
        Some(container(pelicano::opaque(janela)).width(Length::FILL)
            .align_x(pelicano::Align::End)
            .padding(pelicano::Padding { top: (14.0 + d.y).max(0.0),
                right: (26.0 + d.x).max(0.0), bottom: 0.0, left: 0.0 })
            .into())
    }

    /// Move a janela pelo deslocamento do cursor desde o último evento.
    pub(super) fn arrastar_busca(&mut self, cursor: pelicano::Point) {
        let Some(anterior) = self.busca.arrasto else { return };
        if let Some(antes) = anterior {
            let d = &mut self.busca.deslocamento;
            d.x = (d.x - (cursor.x - antes.x)).max(-26.0);
            d.y = (d.y + (cursor.y - antes.y)).max(-14.0);
        }
        self.busca.arrasto = Some(Some(cursor));
    }

    /// Lista do "Localizar tudo": Planilha · Célula · Valor · Fórmula, e um
    /// clique leva até a célula. Até 200 linhas, para não montar milhões de widgets.
    fn view_busca_resultados(&self) -> Element<'_, Msg> {
        let b = &self.busca;
        let negrito = pelicano::Font { weight: 600, ..Default::default() };
        let colunas = |a: Element<'static, Msg>, c: Element<'static, Msg>,
                       v: Element<'static, Msg>, f: Element<'static, Msg>| row![
            container(a).width(Length::Fixed(110.0)),
            container(c).width(Length::Fixed(62.0)),
            container(v).width(Length::Fill(1)),
            container(f).width(Length::Fill(1)),
        ].spacing(8).align(pelicano::Align::Center);
        let cab = |t: &'static str| text(t).size(11.5).font(negrito)
            .color(COR_TEXTO_MUTED()).into();
        let cabecalho = container(colunas(cab(tr!("Planilha")), cab(tr!("Célula")), cab(tr!("Valor")), cab(tr!("Fórmula"))))
            .padding([5, 8]).width(Length::FILL)
            .style(|_: &pelicano::Theme| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL_FAIXA())),
                border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [6.0; 4] },
                ..Default::default()
            });
        let resumo = if b.total == 0 {
            tr!("Nenhuma ocorrência encontrada.").to_owned()
        } else if b.total > b.resultados.len() {
            trf!("{} célula(s) encontrada(s); mostrando as primeiras {}.", b.total, b.resultados.len())
        } else { trf!("{} célula(s) encontrada(s).", b.total) };

        let mut lista = column(Vec::new()).spacing(1);
        for (indice, item) in b.resultados.iter().enumerate() {
            let Some(aba) = self.workbook.aba(item.aba) else { continue };
            let bruto = aba.sheet.get_raw(item.linha, item.coluna);
            let valor = feathersheet_core::celulas::values::to_text(
                &self.workbook.valor(item.aba, item.linha, item.coluna));
            let formula = if bruto.starts_with('=') { bruto } else { String::new() };
            let celula = |t: String| -> Element<'static, Msg> { text(t).size(12).into() };
            let ativa = item.aba == self.folha
                && (item.linha, item.coluna) == (self.linha, self.coluna);
            lista = lista.push(button(colunas(
                    celula(cortar(&aba.sheet.name, 15)),
                    celula(cell_name(item.linha, item.coluna)),
                    celula(cortar(&valor, 26)),
                    celula(cortar(&formula, 26)),
                ))
                .width(Length::FILL).padding([4, 8])
                .style(estilo_botao_pequeno(ativa))
                .on_press(Msg::BuscaIrPara(indice)));
        }
        let mut bloco = column![
            text(resumo).size(11.5).color(COR_TEXTO_MUTED()),
        ].spacing(6);
        if !b.resultados.is_empty() {
            bloco = bloco.push(cabecalho)
                .push(container(rolagem(lista)).height(Length::Fixed(180.0)));
        }
        bloco.into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::Value;

    #[test]
    fn pesquisar_por_colunas_visita_a_coluna_inteira_antes_da_seguinte() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 1, "x".into());
        app.workbook.escrever_celula(0, 3, 0, "x".into());
        app.busca.texto = "x".into();
        let _ = app.localizar_direcao(false);
        assert_eq!((app.linha, app.coluna), (0, 1), "por linhas: B1 vem antes de A4");
        app.trocar_selecao(0, 0);
        let _ = app.update(Msg::BuscaPorColunas(true));
        let _ = app.localizar_direcao(false);
        assert_eq!((app.linha, app.coluna), (3, 0), "por colunas: A4 vem antes de B1");
    }

    #[test]
    fn janela_abre_na_aba_pedida_e_monta_com_opcoes_e_lista_nos_dois_temas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "=1+1".into());
        let _ = app.update(Msg::BuscaAba(true));
        assert!(app.busca.aberta && app.busca.substituir);
        let _ = app.update(Msg::BuscaAba(false));
        assert!(!app.busca.substituir);
        let _ = app.update(Msg::BuscaMaisOpcoes);
        app.busca.texto = "2".into();
        app.localizar_todos();
        assert_eq!(app.busca.resultados.len(), 1);
        for escuro in [false, true] {
            feather_ui::tema::definir_tema(escuro, 0);
            assert!(app.view_busca().is_some());
        }
        let _ = app.update(Msg::FecharBusca);
        assert!(app.view_busca().is_none());
    }

    #[test]
    fn janela_de_busca_segue_o_cursor_ao_arrastar_pelo_titulo() {
        let mut app = App::default();
        let _ = app.update(Msg::BuscaAba(false));
        let _ = app.update(Msg::BuscaArrastoInicio);
        let _ = app.update(Msg::BuscaArrastoMover(pelicano::Point::new(500.0, 100.0)));
        let _ = app.update(Msg::BuscaArrastoMover(pelicano::Point::new(380.0, 160.0)));
        let _ = app.update(Msg::BuscaArrastoFim);
        // 120 px para a esquerda e 60 para baixo; soltar encerra o arrasto.
        assert_eq!(app.busca.deslocamento, crate::compat::Vector::new(120.0, 60.0));
        let _ = app.update(Msg::BuscaArrastoMover(pelicano::Point::new(0.0, 0.0)));
        assert_eq!(app.busca.deslocamento, crate::compat::Vector::new(120.0, 60.0));
    }

    #[test]
    fn localizar_em_pasta_navega_duas_abas_e_volta_para_anterior() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "mesmo".into());
        let outra = app.workbook.criar_aba("Dados");
        app.workbook.escrever_celula(outra, 2, 3, "mesmo".into());
        app.busca.texto = "mesmo".into();
        app.busca.pasta_inteira = true;
        let _ = app.localizar_direcao(false);
        assert_eq!((app.folha, app.linha, app.coluna), (outra, 2, 3));
        app.localizar_direcao(true);
        assert_eq!((app.folha, app.linha, app.coluna), (0, 0, 0));
        app.localizar_direcao(true);
        assert_eq!((app.folha, app.linha, app.coluna), (outra, 2, 3));
    }

    #[test]
    fn busca_na_pasta_pula_aba_oculta_e_limita_resultados_visuais() {
        let mut app = App::default();
        for i in 0..205 {
            app.workbook.escrever_celula(0, i, 0, "azul".into());
        }
        let outra = app.workbook.criar_aba("Oculta");
        app.workbook.escrever_celula(outra, 0, 0, "azul".into());
        assert!(app.workbook.ocultar_aba(outra).is_ok());
        app.busca.texto = "azul".into();
        app.busca.pasta_inteira = true;
        app.localizar_todos();
        assert_eq!(app.busca.total, 205);
        assert_eq!(app.busca.resultados.len(), 200);
        assert!(app.busca.resultados.iter().all(|m| m.aba == 0));
    }

    #[test]
    fn substituir_so_celula_atual_sem_alterar_outras_e_desfaz() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "banana banana".into());
        app.workbook.escrever_celula(0, 1, 0, "banana banana".into());
        app.busca.texto = "banana".into();
        app.busca.substituicao = "pera".into();
        app.substituir_atual();
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("pera pera".into()));
        assert_eq!(app.workbook.valor(0, 1, 0), Value::Text("banana banana".into()));
        assert_eq!((app.linha, app.coluna), (1, 0));
        app.historico(false);
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("banana banana".into()));
    }

    #[test]
    fn substituir_tudo_na_pasta_preserva_undo_individual_por_aba() {
        let mut app = App::default();
        let b = app.workbook.criar_aba("Dados");
        app.workbook.escrever_celula(0, 0, 0, "antigo".into());
        app.workbook.escrever_celula(b, 0, 0, "antigo".into());
        app.busca.texto = "antigo".into();
        app.busca.substituicao = "novo".into();
        app.busca.pasta_inteira = true;
        app.substituir_tudo();
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("novo".into()));
        assert_eq!(app.workbook.valor(b, 0, 0), Value::Text("novo".into()));
        assert!(app.status.contains("desfazer por aba"));
        // O histórico de cada Sheet não depende do índice da ativa.
        assert!(app.workbook.aba_mut(b).unwrap().sheet.undo());
        assert_eq!(app.workbook.valor(b, 0, 0), Value::Text("antigo".into()));
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("novo".into()));
    }

    #[test]
    fn ordenacao_respeita_cabecalho_e_desfaz_com_estilo() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("Nome\tValor\nb\t2\na\t1".into()));
        let _ = app.update(Msg::IrPara("A1:B3".into()));
        let _ = app.update(Msg::CabecalhoDados(true));
        let _ = app.update(Msg::Ordenar(true));
        assert_eq!(app.workbook.valor(0,0,0),Value::Text("Nome".into()));
        assert_eq!(app.workbook.valor(0,1,0),Value::Text("a".into()));
        assert_eq!(app.workbook.valor(0,1,1),Value::Number(1.));
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0,1,0),Value::Text("b".into()));
    }

    #[test]
    fn busca_calcula_referencia_a_outra_aba_e_substitui_com_undo() {
        let mut app = App::default();
        let outra = app.workbook.criar_aba("Dados");
        app.workbook.escrever_celula(outra,0,0,"42".into());
        let _ = app.update(Msg::Colar("=Dados!A1\ntexto texto".into()));
        let _ = app.update(Msg::BuscaTexto("42".into()));
        let _ = app.update(Msg::Localizar);
        assert_eq!((app.linha,app.coluna),(0,0));
        let _ = app.update(Msg::BuscaTexto("texto".into()));
        let _ = app.update(Msg::Substituicao("novo".into()));
        let _ = app.update(Msg::SubstituirTudo);
        assert_eq!(app.workbook.valor(0,1,0),Value::Text("novo novo".into()));
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0,1,0),Value::Text("texto texto".into()));
    }

    #[test]
    fn dividir_texto_aceita_selecao_acima_de_cem_mil_linhas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "primeiro;ES".into());
        app.workbook.escrever_celula(0, 100_000, 0, "ultimo;RJ".into());
        app.update(Msg::IrPara("A1:A100001".into()));
        app.update(Msg::FaixaOpcao("dividir_texto", ";"));
        assert_eq!(app.workbook.valor(0, 0, 1), Value::Text("ES".into()));
        assert_eq!(app.workbook.valor(0, 100_000, 1), Value::Text("RJ".into()));
        assert!(!app.status.contains("100.000"));
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0, 100_000, 0), Value::Text("ultimo;RJ".into()));
        assert_eq!(app.workbook.valor(0, 0, 1), Value::Empty);
        assert_eq!(app.workbook.valor(0, 100_000, 1), Value::Empty);
    }

    #[test]
    fn dividir_em_lote_nao_modifica_formula_sem_separador_ou_linha_sem_divisao() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "A;B".into());
        app.workbook.escrever_celula(0, 1, 0, "=1+2".into());
        app.workbook.escrever_celula(0, 2, 0, "sem divisor".into());
        app.update(Msg::IrPara("A1:A3".into()));
        app.update(Msg::FaixaOpcao("dividir_texto", ";"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1, 0), "=1+2");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(2, 0), "sem divisor");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 1), "B");
        assert!(app.status.contains("1 linha(s)"));
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "A;B");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1, 0), "=1+2");
    }

    #[test]
    fn dividir_rapido_preserva_unicode_e_campos_vazios() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "São🐦Vitória🐦".into());
        // Separador fora da lista pronta entra pelo "Outro…" do menu.
        app.update(Msg::FaixaOpcao("dividir_texto", "Outro…"));
        app.update(Msg::DividirTextoOutro("🐦".into()));
        app.update(Msg::DividirTextoAplicar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "São");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 1), "Vitória");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 2), "");
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "São🐦Vitória🐦");
    }

    #[test]
    fn dividir_texto_detecta_conflito_em_lote_tardio_sem_escrever_no_primeiro() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "A,B".into());
        app.workbook.escrever_celula(0, 2049, 0, "X,Y".into());
        app.workbook.escrever_celula(0, 2049, 1, "ocupado".into());
        app.update(Msg::IrPara("A1:A2050".into()));
        app.update(Msg::FaixaOpcao("dividir_texto", ","));
        assert!(app.status.contains("B2050"));
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("A,B".into()));
        assert_eq!(app.workbook.valor(0, 0, 1), Value::Empty);
    }

    #[test]
    #[ignore = "teste de volume; executar manualmente com --ignored"]
    fn dividir_texto_mais_de_cem_mil_celulas_resultantes() {
        let mut app = App::default();
        app.workbook.editar(0, |sheet| {
            for r in 0..50_001 {
                sheet.set_cell(r, 0, "a;b;c");
            }
        });
        app.update(Msg::IrPara("A1:A50001".into()));
        app.update(Msg::FaixaOpcao("dividir_texto", ";"));
        assert_eq!(app.workbook.valor(0, 50_000, 2), Value::Text("c".into()));
        assert!(!app.status.contains("100.000"));
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0, 50_000, 0), Value::Text("a;b;c".into()));
        assert_eq!(app.workbook.valor(0, 50_000, 2), Value::Empty);
    }

    #[test]
    fn dividir_texto_ponto_virgula_respeita_undo_unico() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("Ana;Dev;ES\nBia;Eng;RJ".into()));
        let _ = app.update(Msg::IrPara("A1:A2".into()));
        let _ = app.update(Msg::FaixaOpcao("dividir_texto", ";"));
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("Ana".into()));
        assert_eq!(app.workbook.valor(0, 0, 1), Value::Text("Dev".into()));
        assert_eq!(app.workbook.valor(0, 1, 2), Value::Text("RJ".into()));
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("Ana;Dev;ES".into()));
        assert_eq!(app.workbook.valor(0, 0, 1), Value::Empty);
    }

    #[test]
    fn dividir_texto_personalizado_aceita_token_de_varios_caracteres() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "a||b||c".into());
        let _ = app.update(Msg::FaixaOpcao("dividir_texto", "Outro…"));
        assert!(app.divisao_personalizada);
        let _ = app.update(Msg::DividirTextoOutro("||".into()));
        let _ = app.update(Msg::DividirTextoAplicar);
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("a".into()));
        assert_eq!(app.workbook.valor(0, 0, 2), Value::Text("c".into()));
    }

    #[test]
    fn dividir_texto_nao_sobrescreve_coluna_vizinha() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "a,b".into());
        app.workbook.escrever_celula(0, 0, 1, "existente".into());
        let _ = app.update(Msg::FaixaOpcao("dividir_texto", ","));
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("a,b".into()));
        assert_eq!(app.workbook.valor(0, 0, 1), Value::Text("existente".into()));
        assert!(app.status.contains("B1"));
    }

    #[test]
    fn dividir_texto_aceita_ponto_e_espaco() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "A.B.C".into());
        let _ = app.update(Msg::FaixaOpcao("dividir_texto", "."));
        assert_eq!(app.workbook.valor(0, 0, 2), Value::Text("C".into()));
        let _ = app.update(Msg::Desfazer);
        app.workbook.escrever_celula(0, 0, 0, "A B C".into());
        let _ = app.update(Msg::FaixaOpcao("dividir_texto", "Espaço"));
        assert_eq!(app.workbook.valor(0, 0, 2), Value::Text("C".into()));
    }

    #[test]
    fn remover_repetidas_considera_linha_inteira() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("a\t1\na\t2\na\t1".into()));
        let _ = app.update(Msg::IrPara("A1:B3".into()));
        let _ = app.update(Msg::RemoverDuplicados);
        // O clique abre o diálogo; os dados só mudam após confirmar.
        assert_eq!(app.workbook.valor(0,2,1),Value::Number(1.));
        let _ = app.update(Msg::Duplicados(EventoDuplicados::Confirmar));
        assert_eq!(app.workbook.valor(0,1,1),Value::Number(2.));
        assert_eq!(app.workbook.valor(0,2,0),Value::Empty);
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0,2,1),Value::Number(1.));
    }
}
