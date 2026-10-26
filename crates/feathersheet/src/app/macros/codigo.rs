//! Ferramentas do código das macros (Alt+F11), 14/10/2026:
//! busca de macro pelo nome no Explorador, Localizar/Substituir (Ctrl+F /
//! Ctrl+H) dentro do código, desfazer/refazer do editor e a posição
//! "Ln X, Col Y". A parte de desenho fica em `editor_visual/codigo_view.rs`.
//!
//! No pelicano o `Content` tem desfazer próprio (digitação seguida vira UMA
//! ação; "Substituir todos" vai por `map_lines`, uma ação só). O editor fica
//! dentro de uma rolagem nossa, com a altura exata do texto (linha de altura
//! fixa), para a calha de números andar junto.
use super::*;
use pelicano::Content;

/// Altura fixa da linha do código (px). A calha de números usa a mesma.
pub(in crate::app) const ALTURA_LINHA: f32 = 18.0;
pub(in crate::app) const TAMANHO_FONTE: f32 = 13.0;
/// Margem interna do editor: vertical e horizontal.
pub(in crate::app) const MARGEM_V: f32 = 10.0;
pub(in crate::app) const MARGEM_H: f32 = 12.0;
/// Largura estimada de um caractere monoespaçado de 13 px (DejaVu Sans Mono
/// dá 7,8; sobra um pouco para não cortar a linha mais comprida).
pub(in crate::app) const LARGURA_CARACTERE: f32 = 8.0;
/// Teto de ocorrências contadas e destacadas.
const TETO_OCORRENCIAS: usize = 10_000;
pub(in crate::app) fn id_rolagem() -> &'static str { "macro-codigo-vertical" }
pub(in crate::app) fn id_rolagem_h() -> &'static str { "macro-codigo-horizontal" }
pub(in crate::app) fn id_campo_busca() -> &'static str { "macro-busca-codigo" }
pub(in crate::app) fn id_campo_troca() -> &'static str { "macro-busca-troca" }
pub(in crate::app) fn id_campo_projeto() -> &'static str { "macro-busca-projeto" }
/// Id do `text_editor` do código (o foco programático vai direto nele).
pub(in crate::app) const ID_AREA_CODIGO: &str = "macro-codigo-area";

#[derive(Debug, Clone)]
pub(in crate::app) struct EstadoCodigo {
    /// Busca do Explorador de projeto (nome do módulo ou da função).
    pub filtro_projeto: String,
    /// Barra de Localizar aberta; `substituir` mostra a segunda linha.
    pub busca_aberta: bool,
    pub substituir: bool,
    pub consulta: String,
    pub troca: String,
    pub maiusculas: bool,
    pub palavra: bool,
    pub todos_modulos: bool,
    /// Ocorrência atual (linha, coluna), ambas a partir de 0, em caracteres.
    pub atual: Option<(usize, usize)>,
    /// De onde a busca parte enquanto se digita a consulta.
    pub origem: (usize, usize),
    /// Shift pressionado (Shift+Enter no campo volta uma ocorrência).
    pub shift: bool,
    /// O foco está num campo da barra (Ctrl+Z não mexe no código).
    pub campo_focado: bool,
    /// Rolagem vertical e horizontal: (deslocamento, tamanho visível).
    pub rolagem_v: Option<(f32, f32)>,
    pub rolagem_h: Option<(f32, f32)>,
    /// Tamanho da janela do editor (largura mínima do código).
    pub janela: pelicano::Size,
    /// Aviso curto da barra (ex.: "5 substituições").
    pub aviso: String,
}

impl Default for EstadoCodigo {
    fn default() -> Self {
        Self {
            filtro_projeto: String::new(), busca_aberta: false, substituir: false,
            consulta: String::new(), troca: String::new(), maiusculas: false,
            palavra: false, todos_modulos: false, atual: None, origem: (0, 0),
            shift: false, campo_focado: false, rolagem_v: None, rolagem_h: None,
            janela: pelicano::Size::new(1180.0, 760.0), aviso: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum EventoCodigo {
    FiltroProjeto(String),
    /// Enter na busca do Explorador: abre o primeiro resultado.
    FiltroAbrirPrimeiro,
    FocarProjeto,
    /// Abre o módulo e põe o cursor na linha (função achada pelo nome).
    IrParaFuncao(String, usize),
    /// Ctrl+F (false) ou Ctrl+H (true).
    Abrir(bool),
    Fechar,
    Consulta(String),
    Troca(String),
    Proximo,
    Anterior,
    /// Enter no campo de busca: Shift decide a direção.
    Enter,
    AlternarMaiusculas,
    AlternarPalavra,
    AlternarTodos,
    AlternarSubstituir,
    Substituir,
    SubstituirTodos,
    /// Resultado de "em todos os módulos": módulo, linha, coluna.
    IrPara(String, usize, usize),
    Desfazer,
    Refazer,
    Rolou(pelicano::Viewport),
    RolouH(pelicano::Viewport),
    Modificadores(keyboard::Modifiers),
}

// ---------------------------------------------------------------- funções puras

pub(in crate::app) use feathersheet_macros::procedimentos::funcoes_do_modulo;

/// Um módulo na busca do Explorador, com as funções que bateram.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::app) struct ItemFiltro {
    pub modulo: String,
    pub auto: bool,
    pub nome_bate: bool,
    pub funcoes: Vec<(String, usize)>,
}

/// Filtra os módulos pelo nome do módulo ou de uma função dele, sem
/// diferenciar acento nem maiúscula (`feather_ui::ajuda::normalizar`).
pub(in crate::app) fn filtrar_modulos(macros: &[feather_macros::Macro], filtro: &str) -> Vec<ItemFiltro> {
    let alvo = feather_ui::ajuda::normalizar(filtro.trim());
    macros.iter().filter_map(|m| {
        if alvo.is_empty() {
            return Some(ItemFiltro { modulo: m.name.clone(), auto: m.auto, nome_bate: true, funcoes: Vec::new() });
        }
        let nome_bate = feather_ui::ajuda::normalizar(&m.name).contains(&alvo);
        let funcoes: Vec<_> = funcoes_do_modulo(&m.code).into_iter()
            .filter(|(f, _)| feather_ui::ajuda::normalizar(f).contains(&alvo)).collect();
        (nome_bate || !funcoes.is_empty()).then(|| ItemFiltro {
            modulo: m.name.clone(), auto: m.auto, nome_bate, funcoes,
        })
    }).collect()
}

/// Uma ocorrência no código: linha e coluna a partir de 0, tamanho em caracteres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::app) struct Ocorrencia { pub linha: usize, pub coluna: usize, pub tamanho: usize }

fn parte_de_palavra(c: char) -> bool { c.is_alphanumeric() || c == '_' || c == '$' }

/// Todas as ocorrências (sem sobreposição), linha a linha.
pub(in crate::app) fn ocorrencias(texto: &str, consulta: &str, maiusculas: bool, palavra: bool) -> Vec<Ocorrencia> {
    let q: Vec<char> = consulta.chars().collect();
    if q.is_empty() || q.contains(&'\n') { return Vec::new(); }
    let igual = |a: char, b: char| a == b || (!maiusculas && a.to_lowercase().eq(b.to_lowercase()));
    let mut saida = Vec::new();
    for (l, linha) in texto.split('\n').enumerate() {
        let cs: Vec<char> = linha.chars().collect();
        let mut i = 0;
        while i + q.len() <= cs.len() {
            let bate = cs[i..i + q.len()].iter().zip(&q).all(|(a, b)| igual(*a, *b))
                && (!palavra || (i.checked_sub(1).is_none_or(|p| !parte_de_palavra(cs[p]))
                    && cs.get(i + q.len()).is_none_or(|c| !parte_de_palavra(*c))));
            if bate {
                saida.push(Ocorrencia { linha: l, coluna: i, tamanho: q.len() });
                if saida.len() >= TETO_OCORRENCIAS { return saida; }
                i += q.len();
            } else { i += 1; }
        }
    }
    saida
}

/// Troca todas as ocorrências; devolve o texto novo e quantas trocou.
pub(in crate::app) fn substituir_todas(texto: &str, achadas: &[Ocorrencia], troca: &str) -> (String, usize) {
    let mut linhas: Vec<String> = texto.split('\n').map(str::to_owned).collect();
    // De trás para frente: a troca não desloca as colunas que faltam.
    for o in achadas.iter().rev() {
        let Some(linha) = linhas.get_mut(o.linha) else { continue };
        let cs: Vec<char> = linha.chars().collect();
        if o.coluna + o.tamanho > cs.len() { continue; }
        let novo: String = cs[..o.coluna].iter().collect::<String>() + troca
            + &cs[o.coluna + o.tamanho..].iter().collect::<String>();
        *linha = novo;
    }
    (linhas.join("\n"), achadas.len())
}

/// Texto exato do editor, linha a linha. O `Content::text()` do pelicano já
/// devolve as quebras como estão ("abc" e "abc" + linha vazia saem diferentes).
pub(in crate::app) fn texto_exato(conteudo: &Content) -> String {
    conteudo.text()
}

/// Recria o conteúdo a partir do [`texto_exato`].
#[cfg(test)]
pub(in crate::app) fn conteudo_de(texto: &str) -> Content {
    Content::with_text(texto)
}

/// Linha e coluna do cursor, contadas a partir de 1, em caracteres.
pub(in crate::app) fn linha_coluna(conteudo: &Content) -> (usize, usize) {
    conteudo.cursor()
}

/// Põe o cursor no começo da linha (a partir de 0).
// shortcut: o `Content` do pelicano só posiciona no começo da linha (`go_to_line`); a coluna
// é ignorada. Todos os chamadores de hoje usam coluna 0, menos o "Substituir todos", que volta
// ao começo da linha do cursor. Melhorar quando o pelicano tiver `set_cursor(linha, coluna)`.
pub(in crate::app) fn posicionar(conteudo: &Content, linha: usize, _coluna: usize) {
    let linha = linha.min(conteudo.line_count().saturating_sub(1));
    conteudo.go_to_line(linha + 1);
}

/// Seleciona `tamanho` caracteres a partir da posição: vai ao começo da linha
/// e anda com o `find_next` (texto exato do trecho) até a ocorrência pedida.
// shortcut: antes do primeiro desenho o editor não está montado e o `find_next` não seleciona
// (só o cursor vai para a linha); melhorar quando o pelicano tiver `select(range)`.
pub(in crate::app) fn selecionar(conteudo: &Content, o: Ocorrencia) {
    posicionar(conteudo, o.linha, 0);
    let Some(texto) = conteudo.line(o.linha + 1) else { return };
    let trecho: String = texto.chars().skip(o.coluna).take(o.tamanho).collect();
    if trecho.is_empty() { return; }
    let alvo = (o.linha + 1, o.coluna + o.tamanho + 1);
    for _ in 0..=texto.chars().count() {
        if !conteudo.find_next(&trecho, true) || conteudo.cursor() == alvo || conteudo.cursor().0 != o.linha + 1 {
            break;
        }
    }
}

/// Texto da seleção, se for de uma linha só (preenche a busca ao abrir).
fn selecao_de_uma_linha(conteudo: &Content) -> Option<String> {
    conteudo.selection().filter(|s| !s.is_empty() && !s.contains('\n'))
}

// ---------------------------------------------------------------- foco no editor

/// Foca o `text_editor` do código (no pelicano ele tem Id próprio).
fn focar_codigo() -> Task<Msg> {
    Task::focus(ID_AREA_CODIGO)
}

// ---------------------------------------------------------------- App

impl App {
    /// Ocorrências da consulta no código aberto.
    pub(in crate::app) fn ocorrencias_atuais(&self) -> Vec<Ocorrencia> {
        let c = &self.macro_visual.codigo;
        if !c.busca_aberta { return Vec::new(); }
        ocorrencias(&texto_exato(&self.macro_codigo), &c.consulta, c.maiusculas, c.palavra)
    }

    /// Resultados em todos os módulos: (módulo, ocorrência, trecho da linha).
    /// O módulo aberto usa o texto do editor, não o guardado.
    pub(in crate::app) fn ocorrencias_em_todos(&self) -> Vec<(String, Ocorrencia, String)> {
        let c = &self.macro_visual.codigo;
        let mut saida = Vec::new();
        let mut modulos: Vec<(String, String)> = self.macro_catalogo.macros.iter()
            .map(|m| (m.name.clone(), m.code.clone())).collect();
        match modulos.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(&self.macro_nome)) {
            Some(m) => m.1 = self.macro_codigo.text(),
            None => modulos.insert(0, (self.macro_nome.clone(), self.macro_codigo.text())),
        }
        for (nome, codigo) in modulos {
            let linhas: Vec<&str> = codigo.split('\n').collect();
            for o in ocorrencias(&codigo, &c.consulta, c.maiusculas, c.palavra) {
                let trecho = linhas.get(o.linha).map_or(String::new(), |l| l.trim().chars().take(80).collect());
                saida.push((nome.clone(), o, trecho));
                if saida.len() >= 500 { return saida; }
            }
        }
        saida
    }

    /// Desfaz no `Content` (ele guarda o próprio desfazer; trocar de documento
    /// cria outro `Content`, então o desfazer não atravessa de um módulo para outro).
    pub(in crate::app) fn desfazer_codigo(&mut self) -> bool {
        if !self.macro_codigo.can_undo() { return false; }
        self.macro_codigo.undo();
        true
    }

    pub(in crate::app) fn refazer_codigo(&mut self) -> bool {
        if !self.macro_codigo.can_redo() { return false; }
        self.macro_codigo.redo();
        true
    }

    /// O `text_editor` já editou o `Content` (desfazer incluso): só rola até o cursor.
    // shortcut: o pelicano não avisa clique no editor (o Iced mandava `Action::Click`), então o
    // `campo_focado` só volta a falso ao editar o código; clicar no código depois da barra de
    // busca e apertar Ctrl+Z sem digitar não desfaz. Melhorar se o editor ganhar `on_focus`.
    pub(in crate::app) fn editar_codigo_macro(&mut self) -> Task<Msg> {
        self.macro_visual.codigo.campo_focado = false;
        self.rolar_ate_cursor()
    }

    /// Rola o código até a linha/coluna do cursor, se estiver fora da vista.
    pub(in crate::app) fn rolar_ate_cursor(&self) -> Task<Msg> {
        let (l, c) = linha_coluna(&self.macro_codigo);
        self.rolar_ate(l - 1, c - 1)
    }

    fn rolar_ate(&self, linha: usize, coluna: usize) -> Task<Msg> {
        let estado = &self.macro_visual.codigo;
        let mut tarefas = Vec::new();
        let topo = MARGEM_V + linha as f32 * ALTURA_LINHA;
        let (desl, altura) = estado.rolagem_v.unwrap_or((0.0, f32::INFINITY));
        let alvo_y = if topo < desl {
            Some((topo - ALTURA_LINHA).max(0.0))
        } else if topo + ALTURA_LINHA > desl + altura {
            Some(topo + 2.0 * ALTURA_LINHA - altura)
        } else { None };
        if let Some(y) = alvo_y {
            tarefas.push(Task::scroll_to(id_rolagem(), pelicano::Point::new(0.0, y)));
        }
        let x = MARGEM_H + coluna as f32 * LARGURA_CARACTERE;
        if let Some((desl_x, largura)) = estado.rolagem_h {
            let alvo_x = if x < desl_x + MARGEM_H {
                Some((x - 4.0 * LARGURA_CARACTERE).max(0.0))
            } else if x + 2.0 * LARGURA_CARACTERE > desl_x + largura {
                Some(x + 6.0 * LARGURA_CARACTERE - largura)
            } else { None };
            if let Some(x) = alvo_x {
                tarefas.push(Task::scroll_to(id_rolagem_h(), pelicano::Point::new(x, 0.0)));
            }
        }
        Task::batch(tarefas)
    }

    /// Torna `o` a ocorrência atual: seleciona no editor e rola até ela.
    fn ir_para_ocorrencia(&mut self, o: Ocorrencia) -> Task<Msg> {
        self.macro_visual.codigo.atual = Some((o.linha, o.coluna));
        selecionar(&self.macro_codigo, o);
        self.rolar_ate(o.linha, o.coluna)
    }

    /// Próxima (ou anterior) ocorrência a partir da atual ou do cursor; dá a volta.
    fn andar_busca(&mut self, para_frente: bool) -> Task<Msg> {
        let achadas = self.ocorrencias_atuais();
        if achadas.is_empty() { return Task::none(); }
        let referencia = self.macro_visual.codigo.atual.unwrap_or_else(|| {
            let (l, c) = linha_coluna(&self.macro_codigo);
            (l - 1, c - 1)
        });
        let atual_valida = self.macro_visual.codigo.atual.is_some();
        let pos = |o: &Ocorrencia| (o.linha, o.coluna);
        let escolhida = if para_frente {
            achadas.iter().find(|o| if atual_valida { pos(o) > referencia } else { pos(o) >= referencia })
                .or(achadas.first())
        } else {
            achadas.iter().rev().find(|o| pos(o) < referencia).or(achadas.last())
        };
        match escolhida.copied() { Some(o) => self.ir_para_ocorrencia(o), None => Task::none() }
    }

    /// Recalcula a ocorrência atual depois de mudar a consulta ou as opções.
    fn reprocurar(&mut self) -> Task<Msg> {
        let origem = self.macro_visual.codigo.origem;
        let achadas = self.ocorrencias_atuais();
        let escolhida = achadas.iter().find(|o| (o.linha, o.coluna) >= origem).or(achadas.first()).copied();
        self.macro_visual.codigo.aviso.clear();
        match escolhida {
            Some(o) => self.ir_para_ocorrencia(o),
            None => { self.macro_visual.codigo.atual = None; Task::none() }
        }
    }

    pub(in crate::app) fn abrir_busca_codigo(&mut self, substituir: bool) -> Task<Msg> {
        if !self.macro_visual.documento.starts_with("modulo:") {
            self.macro_saida = tr!("Abra um módulo JavaScript para localizar no código.").into();
            return Task::none();
        }
        let selecionado = selecao_de_uma_linha(&self.macro_codigo);
        let (l, c) = linha_coluna(&self.macro_codigo);
        let estado = &mut self.macro_visual.codigo;
        estado.busca_aberta = true;
        estado.substituir = substituir;
        estado.campo_focado = true;
        estado.aviso.clear();
        if let Some(s) = selecionado {
            // A seleção começa antes do cursor: procurar a partir do início dela.
            estado.origem = (l - 1, (c - 1).saturating_sub(s.chars().count()));
            estado.consulta = s;
        } else {
            estado.origem = (l - 1, c - 1);
        }
        estado.atual = None;
        let foco = if substituir && !self.macro_visual.codigo.consulta.is_empty() {
            Task::focus(id_campo_troca())
        } else { Task::focus(id_campo_busca()) };
        // shortcut: o pelicano não tem Task para selecionar todo o texto do campo (o Iced tinha
        // `text_input::select_all`); a consulta aparece sem seleção. Acrescentar quando houver.
        let busca = self.reprocurar();
        Task::batch([foco, busca])
    }

    fn substituir_atual_codigo(&mut self) -> Task<Msg> {
        let achadas = self.ocorrencias_atuais();
        let atual = self.macro_visual.codigo.atual
            .and_then(|p| achadas.iter().find(|o| (o.linha, o.coluna) == p).copied());
        let Some(o) = atual else { return self.andar_busca(true) };
        selecionar(&self.macro_codigo, o);
        let troca = self.macro_visual.codigo.troca.clone();
        let tamanho = troca.chars().count();
        // Só troca se a seleção é mesmo a ocorrência (o editor pode não estar montado ainda).
        if self.macro_codigo.selection().is_some_and(|s| s.chars().count() == o.tamanho) {
            // `insert` troca a seleção (vazio apaga), num passo do desfazer.
            self.macro_codigo.insert(&troca);
        }
        // Segue para a próxima depois do texto trocado (a troca pode conter a
        // consulta: não procurar dentro dela).
        self.macro_visual.codigo.origem = (o.linha, o.coluna + tamanho);
        self.reprocurar()
    }

    fn substituir_todas_codigo(&mut self) -> Task<Msg> {
        let achadas = self.ocorrencias_atuais();
        if achadas.is_empty() {
            self.macro_visual.codigo.aviso = tr!("Nenhuma ocorrência").into();
            return Task::none();
        }
        let (l, c) = linha_coluna(&self.macro_codigo);
        let estado = &self.macro_visual.codigo;
        let (consulta, maiusculas, palavra, troca) = (estado.consulta.clone(), estado.maiusculas, estado.palavra, estado.troca.clone());
        let n = achadas.len();
        // Uma ação só no desfazer, por maior que seja a troca: `map_lines` troca
        // linha a linha (a ocorrência nunca atravessa linha) num passo só.
        self.macro_codigo.map_lines(|linha| {
            let nesta = ocorrencias(linha, &consulta, maiusculas, palavra);
            substituir_todas(linha, &nesta, &troca).0
        });
        posicionar(&self.macro_codigo, l - 1, c - 1);
        let estado = &mut self.macro_visual.codigo;
        estado.atual = None;
        estado.aviso = if n == 1 { tr!("1 substituição").into() } else { trf!("{n} substituições") };
        Task::none()
    }

    fn abrir_modulo_na_linha(&mut self, modulo: &str, linha: usize, coluna: usize, tamanho: usize) -> Task<Msg> {
        if !self.macro_nome.eq_ignore_ascii_case(modulo) || !self.macro_visual.documento.starts_with("modulo:") {
            self.evento_macro_visual(EventoVisual::Documento(format!("modulo:{modulo}")));
        }
        if !self.macro_nome.eq_ignore_ascii_case(modulo) { return Task::none(); }
        let o = Ocorrencia { linha, coluna, tamanho };
        if tamanho == 0 {
            posicionar(&self.macro_codigo, linha, coluna);
            self.macro_visual.codigo.campo_focado = false;
            return Task::batch([self.rolar_ate(linha, coluna), focar_codigo()]);
        }
        self.ir_para_ocorrencia(o)
    }

    /// Esc: fecha a barra de busca ou limpa a busca do Explorador.
    /// Devolve `None` quando não havia nada para fechar.
    pub(in crate::app) fn escape_codigo(&mut self) -> Option<Task<Msg>> {
        if self.macro_visual.codigo.busca_aberta {
            return Some(self.evento_codigo_macro(EventoCodigo::Fechar));
        }
        if !self.macro_visual.codigo.filtro_projeto.is_empty() {
            self.macro_visual.codigo.filtro_projeto.clear();
            return Some(Task::none());
        }
        None
    }

    /// `FEATHERSHEET_CAPTURAR_JANELA=busca_macros`: abre o editor com um
    /// módulo de exemplo (só na memória, nada é gravado), a busca do
    /// Explorador com "total", Ctrl+H com "sheet" → "planilha" e o cursor na
    /// linha 9, para fotografar a barra, os destaques e a calha de números.
    pub(in crate::app) fn preparar_captura_busca_macros(&mut self) -> Task<Msg> {
        let exemplo = [
            "// Relatório mensal · JavaScript do FeatherSheet",
            "function calcularTotal(coluna) {",
            "  let total = 0;",
            "  for (let linha = 2; linha <= sheet.lastRow(); linha++) {",
            "    total += Number(sheet.value(coluna + linha)) || 0;",
            "  }",
            "  return total;",
            "}",
            "",
            "const limparResumo = () => sheet.clear('E1:E10');",
            "",
            "limparResumo();",
            "sheet.set('E1', 'Total de vendas');",
            "sheet.set('E2', calcularTotal('B'));",
            "sheet.style('E1:E2', { bold: true });",
            "log('Resumo atualizado: ' + sheet.value('E2'));",
        ].join("\n");
        self.macro_catalogo = feather_macros::Catalogo {
            macros: vec![
                feather_macros::Macro { name: "Relatorio".into(), code: exemplo.clone(), auto: false },
                feather_macros::Macro { name: tr!("Importar").into(), code: "function totalPorMes() {}\n".into(), auto: false },
                feather_macros::Macro { name: tr!("Limpar").into(), code: "sheet.clear('A1:D10');\n".into(), auto: true },
            ],
        };
        self.macro_nome = "Relatorio".into();
        self.macro_visual.documento = "modulo:Relatorio".into();
        self.macro_codigo = Content::with_text(&exemplo);
        self.macro_visual.codigo.filtro_projeto = "TOTAL".into();
        posicionar(&self.macro_codigo, 8, 0);
        let estado = &mut self.macro_visual.codigo;
        // FEATHERSHEET_CAPTURAR_BUSCA troca o termo procurado ("-" deixa a barra
        // fechada; "*" na frente liga "Todos os módulos").
        let termo = std::env::var("FEATHERSHEET_CAPTURAR_BUSCA").unwrap_or_else(|_| "sheet".into());
        let (termo, todos) = match termo.strip_prefix('*') { Some(t) => (t.to_owned(), true), None => (termo, false) };
        estado.todos_modulos = todos;
        estado.busca_aberta = termo != "-";
        estado.substituir = true;
        estado.consulta = if termo == "-" { String::new() } else { termo };
        estado.troca = "planilha".into();
        estado.origem = (8, 0);
        let _ = self.reprocurar();
        self.macro_visual.console = false;
        self.alternar(Auxiliar::Macros)
    }

    pub(in crate::app) fn evento_codigo_macro(&mut self, evento: EventoCodigo) -> Task<Msg> {
        match evento {
            EventoCodigo::FiltroProjeto(t) => { self.macro_visual.codigo.filtro_projeto = t; Task::none() }
            EventoCodigo::FocarProjeto => Task::focus(id_campo_projeto()),
            EventoCodigo::FiltroAbrirPrimeiro => {
                let itens = filtrar_modulos(&self.macro_catalogo.macros, &self.macro_visual.codigo.filtro_projeto);
                let Some(primeiro) = itens.first() else { return Task::none() };
                let linha = if primeiro.nome_bate { None } else { primeiro.funcoes.first().map(|f| f.1) };
                let modulo = primeiro.modulo.clone();
                match linha {
                    Some(l) => self.abrir_modulo_na_linha(&modulo, l, 0, 0),
                    None => {
                        self.evento_macro_visual(EventoVisual::Documento(format!("modulo:{modulo}")));
                        focar_codigo()
                    }
                }
            }
            EventoCodigo::IrParaFuncao(modulo, linha) => self.abrir_modulo_na_linha(&modulo, linha, 0, 0),
            EventoCodigo::Abrir(substituir) => self.abrir_busca_codigo(substituir),
            EventoCodigo::Fechar => {
                let estado = &mut self.macro_visual.codigo;
                estado.busca_aberta = false;
                estado.campo_focado = false;
                estado.aviso.clear();
                // Volta o foco ao código, com a ocorrência atual selecionada.
                focar_codigo()
            }
            EventoCodigo::Consulta(t) => {
                self.macro_visual.codigo.consulta = t;
                self.macro_visual.codigo.campo_focado = true;
                self.reprocurar()
            }
            EventoCodigo::Troca(t) => {
                self.macro_visual.codigo.troca = t;
                self.macro_visual.codigo.campo_focado = true;
                Task::none()
            }
            EventoCodigo::Proximo => self.andar_busca(true),
            EventoCodigo::Anterior => self.andar_busca(false),
            EventoCodigo::Enter => {
                let frente = !self.macro_visual.codigo.shift;
                self.andar_busca(frente)
            }
            EventoCodigo::AlternarMaiusculas => {
                self.macro_visual.codigo.maiusculas ^= true;
                self.reprocurar()
            }
            EventoCodigo::AlternarPalavra => {
                self.macro_visual.codigo.palavra ^= true;
                self.reprocurar()
            }
            EventoCodigo::AlternarTodos => { self.macro_visual.codigo.todos_modulos ^= true; Task::none() }
            EventoCodigo::AlternarSubstituir => {
                self.macro_visual.codigo.substituir ^= true;
                if self.macro_visual.codigo.substituir { Task::focus(id_campo_troca()) } else { Task::none() }
            }
            EventoCodigo::Substituir => self.substituir_atual_codigo(),
            EventoCodigo::SubstituirTodos => self.substituir_todas_codigo(),
            EventoCodigo::IrPara(modulo, linha, coluna) => {
                let tamanho = self.macro_visual.codigo.consulta.chars().count();
                self.abrir_modulo_na_linha(&modulo, linha, coluna, tamanho)
            }
            EventoCodigo::Desfazer => {
                if self.desfazer_codigo() { self.rolar_ate_cursor() } else { Task::none() }
            }
            EventoCodigo::Refazer => {
                if self.refazer_codigo() { self.rolar_ate_cursor() } else { Task::none() }
            }
            EventoCodigo::Rolou(v) => {
                let o = v.offset;
                let primeira = self.macro_visual.codigo.rolagem_v.is_none();
                self.macro_visual.codigo.rolagem_v = Some((o.y, v.bounds.height));
                // Só agora se sabe a altura visível: leva a ocorrência (ou o
                // cursor) para a vista se o editor abriu já com ela lá embaixo.
                match self.macro_visual.codigo.atual.filter(|_| primeira) {
                    Some((l, c)) => self.rolar_ate(l, c),
                    None if primeira => self.rolar_ate_cursor(),
                    None => Task::none(),
                }
            }
            EventoCodigo::RolouH(v) => {
                let o = v.offset;
                self.macro_visual.codigo.rolagem_h = Some((o.x, v.bounds.width));
                Task::none()
            }
            EventoCodigo::Modificadores(m) => { self.macro_visual.codigo.shift = m.shift(); Task::none() }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn modulo(nome: &str, codigo: &str) -> feather_macros::Macro {
        feather_macros::Macro { name: nome.into(), code: codigo.into(), auto: false }
    }

    fn texto(app: &App) -> String {
        texto_exato(&app.macro_codigo)
    }

    /// O `Content` do pelicano só tem cursor e seleção depois do primeiro
    /// desenho: desenha o editor uma vez, sem GPU.
    pub(in crate::app) fn montar(c: &Content) {
        let el: Element<'_, Msg> = pelicano::text_editor(c).wrap(false).into();
        let _ = pelicano::capture(&el, pelicano::Size::new(600.0, 400.0), 1.0, &pelicano::Theme::light(),
            &mut pelicano::Renderer::new(), None);
    }

    fn app_com_codigo(codigo: &str) -> App {
        let mut app = App::default();
        app.macro_catalogo = Default::default();
        app.macro_nome = "Macro1".into();
        app.macro_visual.documento = "modulo:Macro1".into();
        app.macro_codigo = Content::with_text(codigo);
        montar(&app.macro_codigo);
        app
    }

    #[test]
    fn busca_do_projeto_ignora_acento_e_maiuscula_e_acha_funcao() {
        let macros = vec![
            modulo("Relatório", "log(1);"),
            modulo("Importar", "function calcularTotal() {}\nconst limparAbas = () => 1;\nlet x = 3;"),
            modulo("Outro", "async function ÚltimaLinha() {}"),
        ];
        let r = filtrar_modulos(&macros, "RELATORIO");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].modulo, "Relatório");
        assert!(r[0].nome_bate);
        let r = filtrar_modulos(&macros, "limpar");
        assert_eq!(r[0].modulo, "Importar");
        assert!(!r[0].nome_bate);
        assert_eq!(r[0].funcoes, vec![("limparAbas".to_string(), 1)]);
        let r = filtrar_modulos(&macros, "ultima");
        assert_eq!(r[0].funcoes, vec![("ÚltimaLinha".to_string(), 0)]);
        assert_eq!(filtrar_modulos(&macros, "").len(), 3);
        assert!(filtrar_modulos(&macros, "x").iter().all(|i| i.funcoes.is_empty() || i.modulo != "Importar"));
        assert!(filtrar_modulos(&macros, "nada-disso").is_empty());
    }

    #[test]
    fn enter_na_busca_do_projeto_abre_a_primeira_e_esc_limpa() {
        let mut app = app_com_codigo("log(0);");
        app.macro_catalogo.macros = vec![
            modulo("Macro1", "log(0);"),
            modulo("Contas", "log(1);\n\nfunction somarTudo() {}\n"),
        ];
        let _ = app.evento_codigo_macro(EventoCodigo::FiltroProjeto("SOMAR".into()));
        let _ = app.evento_codigo_macro(EventoCodigo::FiltroAbrirPrimeiro);
        assert_eq!(app.macro_visual.documento, "modulo:Contas");
        montar(&app.macro_codigo);
        assert_eq!(linha_coluna(&app.macro_codigo), (3, 1));
        assert!(app.escape_codigo().is_some());
        assert!(app.macro_visual.codigo.filtro_projeto.is_empty());
        assert!(app.escape_codigo().is_none());
    }

    #[test]
    fn ocorrencias_respeitam_maiusculas_e_palavra_inteira() {
        let t = "Total = total + subtotal;\nTOTAL";
        assert_eq!(ocorrencias(t, "total", false, false).len(), 4);
        assert_eq!(ocorrencias(t, "total", true, false).len(), 2);
        let palavra = ocorrencias(t, "total", false, true);
        assert_eq!(palavra, vec![
            Ocorrencia { linha: 0, coluna: 0, tamanho: 5 },
            Ocorrencia { linha: 0, coluna: 8, tamanho: 5 },
            Ocorrencia { linha: 1, coluna: 0, tamanho: 5 },
        ]);
        assert!(ocorrencias(t, "", false, false).is_empty());
        // Acento conta como caractere (coluna em caracteres, não em bytes).
        assert_eq!(ocorrencias("ação ação", "ão", false, false)[1].coluna, 7);
    }

    #[test]
    fn localizar_proximo_e_anterior_com_contagem_e_volta() {
        let mut app = app_com_codigo("a = 1;\nb = a + a;\nlog(a);");
        let _ = app.evento_codigo_macro(EventoCodigo::Abrir(false));
        let _ = app.evento_codigo_macro(EventoCodigo::Consulta("a".into()));
        let achadas = app.ocorrencias_atuais();
        assert_eq!(achadas.len(), 4);
        let indice = |app: &App| {
            let p = app.macro_visual.codigo.atual.unwrap();
            app.ocorrencias_atuais().iter().position(|o| (o.linha, o.coluna) == p).unwrap() + 1
        };
        assert_eq!(indice(&app), 1);
        assert_eq!(app.macro_codigo.selection().as_deref(), Some("a"));
        let _ = app.evento_codigo_macro(EventoCodigo::Proximo);
        assert_eq!(indice(&app), 2);
        assert_eq!(app.macro_visual.codigo.atual, Some((1, 4)));
        assert_eq!(linha_coluna(&app.macro_codigo), (2, 6));
        let _ = app.evento_codigo_macro(EventoCodigo::Proximo);
        let _ = app.evento_codigo_macro(EventoCodigo::Proximo);
        assert_eq!(indice(&app), 4);
        let _ = app.evento_codigo_macro(EventoCodigo::Proximo);
        assert_eq!(indice(&app), 1, "dá a volta para a primeira");
        let _ = app.evento_codigo_macro(EventoCodigo::Anterior);
        assert_eq!(indice(&app), 4, "e para trás também");
        // Shift+Enter volta; Enter avança.
        let _ = app.evento_codigo_macro(EventoCodigo::Modificadores(keyboard::Modifiers::SHIFT));
        let _ = app.evento_codigo_macro(EventoCodigo::Enter);
        assert_eq!(indice(&app), 3);
        let _ = app.evento_codigo_macro(EventoCodigo::Modificadores(keyboard::Modifiers::default()));
        let _ = app.evento_codigo_macro(EventoCodigo::Enter);
        assert_eq!(indice(&app), 4);
        let _ = app.evento_codigo_macro(EventoCodigo::Fechar);
        assert!(!app.macro_visual.codigo.busca_aberta);
    }

    #[test]
    fn abrir_busca_preenche_com_a_selecao() {
        let mut app = app_com_codigo("let valor = 1;\nlog(valor);");
        selecionar(&app.macro_codigo, Ocorrencia { linha: 0, coluna: 4, tamanho: 5 });
        let _ = app.evento_codigo_macro(EventoCodigo::Abrir(true));
        assert_eq!(app.macro_visual.codigo.consulta, "valor");
        assert!(app.macro_visual.codigo.substituir);
        assert_eq!(app.macro_visual.codigo.atual, Some((0, 4)));
    }

    #[test]
    fn substituir_todos_e_um_unico_desfazer() {
        let original = "x = 1;\nx = x + 2;\nlog(x);";
        let mut app = app_com_codigo(original);
        let _ = app.evento_codigo_macro(EventoCodigo::Abrir(true));
        let _ = app.evento_codigo_macro(EventoCodigo::Consulta("x".into()));
        let _ = app.evento_codigo_macro(EventoCodigo::Troca("total".into()));
        let _ = app.evento_codigo_macro(EventoCodigo::SubstituirTodos);
        assert_eq!(texto(&app), "total = 1;\ntotal = total + 2;\nlog(total);");
        assert_eq!(app.macro_visual.codigo.aviso, "4 substituições");
        let _ = app.evento_codigo_macro(EventoCodigo::Desfazer);
        assert_eq!(texto(&app), original);
        let _ = app.evento_codigo_macro(EventoCodigo::Refazer);
        assert!(texto(&app).starts_with("total = 1;"));
    }

    #[test]
    fn substituir_um_troca_a_atual_e_segue() {
        let mut app = app_com_codigo("a a a");
        let _ = app.evento_codigo_macro(EventoCodigo::Abrir(true));
        let _ = app.evento_codigo_macro(EventoCodigo::Consulta("a".into()));
        let _ = app.evento_codigo_macro(EventoCodigo::Troca("bb".into()));
        let _ = app.evento_codigo_macro(EventoCodigo::Substituir);
        assert_eq!(texto(&app), "bb a a");
        assert_eq!(app.macro_visual.codigo.atual, Some((0, 3)));
        let _ = app.evento_codigo_macro(EventoCodigo::Substituir);
        assert_eq!(texto(&app), "bb bb a");
        let _ = app.evento_codigo_macro(EventoCodigo::Desfazer);
        assert_eq!(texto(&app), "bb a a");
    }

    #[test]
    fn desfazer_e_refazer_usam_o_content() {
        let mut app = app_com_codigo("");
        app.macro_codigo.insert("abc");
        let _ = app.editar_codigo_macro();
        app.macro_codigo.insert("\nde");
        assert_eq!(texto(&app), "abc\nde");
        let _ = app.evento_codigo_macro(EventoCodigo::Desfazer);
        assert_eq!(texto(&app), "abc");
        let _ = app.evento_codigo_macro(EventoCodigo::Desfazer);
        assert_eq!(texto(&app), "");
        let _ = app.evento_codigo_macro(EventoCodigo::Refazer);
        assert_eq!(texto(&app), "abc");
        // Outro documento é outro `Content`: o desfazer não atravessa.
        app.macro_codigo = Content::with_text("x");
        assert!(!app.desfazer_codigo());
    }

    #[test]
    fn texto_exato_volta_igual_com_e_sem_linha_vazia_no_fim() {
        for t in ["", "abc", "abc\n", "abc\n\n", "a\nb"] {
            assert_eq!(texto_exato(&conteudo_de(t)), t);
        }
    }

    #[test]
    fn linha_e_coluna_contam_caracteres_a_partir_de_1() {
        let c = Content::with_text("primeira\nação = 2;\n");
        montar(&c);
        assert_eq!(linha_coluna(&c), (1, 1));
        selecionar(&c, Ocorrencia { linha: 1, coluna: 0, tamanho: 4 });
        assert_eq!(linha_coluna(&c), (2, 5), "coluna em caracteres, não em bytes");
        posicionar(&c, 9, 99);
        assert_eq!(linha_coluna(&c).0, c.line_count(), "linha além do fim para na última");
    }

    #[test]
    fn todos_os_modulos_usam_o_texto_do_editor_no_modulo_aberto() {
        let mut app = app_com_codigo("sheet.set('A1', 1);");
        app.macro_catalogo.macros = vec![
            modulo("Macro1", "guardado sem a palavra"),
            modulo("Contas", "log(1);\nsheet.clear('B1');"),
        ];
        let _ = app.evento_codigo_macro(EventoCodigo::Abrir(false));
        let _ = app.evento_codigo_macro(EventoCodigo::Consulta("sheet".into()));
        let todos = app.ocorrencias_em_todos();
        assert_eq!(todos.len(), 2);
        assert_eq!((todos[0].0.as_str(), todos[0].1.linha), ("Macro1", 0));
        assert_eq!((todos[1].0.as_str(), todos[1].1.linha), ("Contas", 1));
        assert_eq!(todos[1].2, "sheet.clear('B1');");
    }
}
