//! Gerenciador de nomes (Fórmulas > Gerenciador), no padrão do Excel:
//! tabela Nome / Valor / Refere-se a, botões Novo, Editar e Excluir e o
//! campo "Refere-se a" editável do nome escolhido. O motor guarda os nomes
//! (`Workbook::definir_nome`); aqui só ficam o formulário e o desenho.
//! Além dos intervalos, há os nomes de valor/fórmula (`Taxa` = `=0,1`) e os de uma aba só (Escopo),
//! que vêm do Excel ou nascem aqui quando "Refere-se a" é uma conta.
use super::*;
use super::dialogo_interno::{dlg, Dialogo};

/// O que a parte de baixo do gerenciador mostra.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) enum Modo {
    /// Tabela + "Refere-se a" do nome escolhido.
    #[default]
    Lista,
    /// Formulário de nome novo.
    Novo,
    /// Formulário editando o nome original.
    Editar(String),
    /// Pedindo confirmação para excluir o nome.
    Excluir(String),
}

#[derive(Default)]
pub(super) struct Estado {
    pub nome: String,
    pub erro: Option<String>,
    /// Nome escolhido na tabela.
    pub selecionado: Option<String>,
    /// Texto do campo "Refere-se a" (do escolhido ou do formulário).
    pub refere: String,
    pub modo: Modo,
    /// Escopo do nome novo: `None` = a pasta inteira; `Some(aba)` = só naquela aba (como o "Escopo" do Excel).
    pub escopo: Option<String>,
}

/// Ações do gerenciador além das mensagens antigas (`NomeCriar`…).
#[derive(Debug, Clone)]
pub enum Evento {
    Selecionar(String),
    Novo,
    Editar,
    Excluir,
    ExcluirConfirmar,
    Refere(String),
    /// Grava o "Refere-se a" editado do nome escolhido.
    Aplicar,
    /// Escopo do nome novo, pela posição na lista (0 = Pasta de trabalho, depois as abas).
    Escopo(usize),
    /// Sai do formulário/confirmação e volta para a tabela.
    Voltar,
}

/// Nome de fórmula não pode ser uma referência A1 válida nem conter espaços.
/// Preserva letras acentuadas como no padrão TypeScript, com limite de 60 chars.
pub(super) fn validar_nome(nome: &str) -> Result<(), &'static str> {
    let nome = nome.trim();
    if nome.is_empty() || nome.chars().count() > 60 {
        return Err(tr!("O nome precisa ter entre 1 e 60 caracteres."));
    }
    let mut caracteres = nome.chars();
    if !caracteres.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        || !caracteres.all(|c| c.is_alphanumeric() || c == '_' || c == '.')
    {
        return Err(tr!("Use letras, números, _ ou .; comece por letra ou _."));
    }
    let posicao_numero = nome.find(|c: char| c.is_ascii_digit());
    if let Some(pos) = posicao_numero {
        let coluna = &nome[..pos];
        let linha = &nome[pos..];
        if let (Some(_), Ok(linha)) = (col_index(coluna), linha.parse::<u32>()) {
            if (1..=MAX_ROWS).contains(&linha) {
                return Err(tr!("Esse nome é um endereço de célula, como A1."));
            }
        }
    }
    if parece_r1c1(nome) {
        return Err(tr!("Esse nome parece um endereço no estilo R1C1 (como R1C1, R ou C2); o Excel não abriria o arquivo."));
    }
    Ok(())
}

/// R, C, RC, R1C1, R2 e o que começa com R ou C seguido de número (R1x, C1_): o Excel lê como referência
/// R1C1 e recusa o arquivo. `Receita` e `CEP` passam.
fn parece_r1c1(nome: &str) -> bool {
    let n = nome.to_ascii_uppercase();
    let mut resto = n.as_str();
    let mut digito = false;
    for letra in ['R', 'C'] {
        if let Some(depois) = resto.strip_prefix(letra) {
            let sem = depois.trim_start_matches(|c: char| c.is_ascii_digit());
            digito |= sem.len() < depois.len();
            resto = sem;
        }
    }
    resto.len() < n.len() && (resto.is_empty() || digito)
}

/// `=Planilha1!$A$1:$B$3`, como o Excel mostra em "Refere-se a".
pub(super) fn texto_refere(area: &Rect) -> String {
    let n = area.normalized();
    let aba = n.sheet.as_deref()
        .map(|a| format!("{}!", feathersheet_core::celulas::refs::quote_sheet(a)))
        .unwrap_or_default();
    let col = feathersheet_core::celulas::refs::col_name;
    let corpo = if n.is_full_col() {
        format!("${}:${}", col(n.c0), col(n.c1))
    } else if n.is_full_row() {
        format!("${}:${}", n.r0 + 1, n.r1 + 1)
    } else if n.r0 == n.r1 && n.c0 == n.c1 {
        format!("${}${}", col(n.c0), n.r0 + 1)
    } else {
        format!("${}${}:${}${}", col(n.c0), n.r0 + 1, col(n.c1), n.r1 + 1)
    };
    format!("={aba}{corpo}")
}

/// Lê "Refere-se a": `=Aba!$A$1:$B$2`, `'Meu plano'!A1`, `B7`, `C:C`.
/// Sem aba, vale a aba `padrao`. A aba precisa existir na pasta.
pub(super) fn ler_refere(
    workbook: &Workbook,
    texto: &str,
    padrao: &str,
) -> Result<Rect, &'static str> {
    let t = texto.trim();
    let t = t.strip_prefix('=').unwrap_or(t).trim();
    if t.is_empty() {
        return Err(tr!("Escreva a referência, como =Planilha1!$A$1:$B$10."));
    }
    // O ! que separa a aba é o último fora de aspas.
    let mut em_aspas = false;
    let mut separador = None;
    let mut iter = t.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        if c == '\'' {
            if em_aspas && iter.peek().is_some_and(|(_, d)| *d == '\'') {
                iter.next();
            } else {
                em_aspas = !em_aspas;
            }
        } else if c == '!' && !em_aspas {
            separador = Some(i);
        }
    }
    let (aba, endereco) = match separador {
        Some(i) => {
            let nome = &t[..i];
            let nome = if nome.len() >= 2 && nome.starts_with('\'') && nome.ends_with('\'') {
                nome[1..nome.len() - 1].replace("''", "'")
            } else {
                nome.to_owned()
            };
            (nome, &t[i + 1..])
        }
        None => (padrao.to_owned(), t),
    };
    let Some(indice) = workbook.indice_por_nome(&aba) else {
        return Err(tr!("A aba dessa referência não existe."));
    };
    let real = workbook.nomes().get(indice).cloned().unwrap_or(aba);
    let limpo: String = endereco.chars().filter(|c| *c != '$').collect();
    let Some(mut area) = feathersheet_core::celulas::refs::parse_range_ref(&limpo) else {
        return Err(tr!("Referência inválida. Use, por exemplo, =Planilha1!$A$1:$B$10."));
    };
    area.sheet = Some(real);
    Ok(area)
}

/// Corta o texto em `max` caracteres com reticências.
fn cortar(texto: &str, max: usize) -> String {
    if texto.chars().count() <= max {
        texto.to_owned()
    } else {
        let mut s: String = texto.chars().take(max.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

/// Uma linha do gerenciador: um intervalo da pasta ([`feathersheet_core::planilha::workbook::Nomeado`]) ou um
/// nome de valor/fórmula ou de uma aba só ([`feathersheet_core::planilha::workbook::NomeFormula`]).
#[derive(Debug, Clone)]
struct LinhaNome {
    /// O que identifica a linha: `Nome` (da pasta) ou `Aba!Nome` (nome não tem "!").
    chave: String,
    nome: String,
    escopo: Option<String>,
    area: Option<Rect>,
    formula: Option<String>,
}

fn chave_do_nome(nome: &str, escopo: Option<&str>) -> String {
    match escopo { Some(aba) => format!("{aba}!{nome}"), None => nome.to_owned() }
}

impl App {
    /// Seleção exatamente igual a um nome definido: a caixa de nome mostra
    /// o nome, como no Excel. Enquanto o usuário digita, nada.
    pub(super) fn nome_da_selecao(&self) -> Option<String> {
        if self.endereco_digitado { return None; }
        let aba = &self.workbook.aba(self.folha)?.sheet.name;
        let area = self.area_selecionada().normalized();
        self.workbook.nomeados().into_iter().find(|n| {
            let a = n.area.normalized();
            n.area.sheet.as_ref().is_none_or(|s| s.eq_ignore_ascii_case(aba))
                && (a.r0, a.c0, a.r1, a.c1) == (area.r0, area.c0, area.r1, area.c1)
        }).map(|n| n.nome)
    }

    /// Os nomes que o gerenciador mostra: os intervalos da pasta e os de valor/fórmula ou de uma aba só.
    fn linhas_de_nomes(&self) -> Vec<LinhaNome> {
        let mut v: Vec<LinhaNome> = self.workbook.nomeados().into_iter().map(|n| LinhaNome {
            chave: n.nome.clone(), nome: n.nome, escopo: None, area: Some(n.area), formula: None,
        }).collect();
        v.extend(self.workbook.nomes_formula().into_iter().map(|n| LinhaNome {
            chave: chave_do_nome(&n.nome, n.aba.as_deref()), area: n.area(), formula: Some(n.formula.clone()),
            nome: n.nome, escopo: n.aba,
        }));
        v.sort_by_key(|l| (l.nome.to_lowercase(), l.escopo.clone()));
        v
    }

    fn linha_de_nome(&self, chave: &str) -> Option<LinhaNome> {
        self.linhas_de_nomes().into_iter().find(|l| l.chave.eq_ignore_ascii_case(chave))
    }

    /// O "Refere-se a" de uma linha: o intervalo, ou a fórmula na língua da interface (`=0.1` em inglês).
    fn refere_da_linha(&self, l: &LinhaNome) -> String {
        match (&l.formula, &l.area) {
            (Some(f), _) => self.formula_para_mostrar(f),
            (None, Some(a)) => texto_refere(a),
            (None, None) => String::new(),
        }
    }

    /// "Refere-se a" digitado: um intervalo (`Ok(Ok(area))`) ou uma fórmula (`Ok(Err("=0,1"))`, na forma
    /// interna). Uma fórmula que não compila é recusada com o motivo.
    fn ler_refere_ou_formula(&self, texto: &str) -> Result<Result<Rect, String>, &'static str> {
        let padrao = self.nome_aba.clone();
        match ler_refere(&self.workbook, texto, &padrao) {
            Ok(area) => Ok(Ok(area)),
            Err(e) => {
                let corpo = texto.trim().strip_prefix('=').unwrap_or(texto.trim()).trim();
                // Sem "=" e sem cara de conta, era para ser uma referência: o erro dela explica melhor.
                if corpo.is_empty() || (!texto.trim().starts_with('=') && corpo.chars().all(|c| c.is_alphanumeric() || "$:!'_ ".contains(c))) {
                    return Err(e);
                }
                let formula = self.formula_para_gravar(&format!("={corpo}"), "");
                match feathersheet_core::formula::parser::parse_formula(formula.strip_prefix('=').unwrap_or(&formula)) {
                    Ok(_) => Ok(Err(formula)),
                    Err(_) => Err(tr!("Fórmula inválida em \"Refere-se a\". Use uma referência (=Planilha1!$A$1) ou uma conta (=0,1).")),
                }
            }
        }
    }

    pub(super) fn abrir_nomes(&mut self, preencher: bool) -> Task<Msg> {
        if !self.confirmar() { return Task::none(); }
        self.menu_aberto = false;
        self.contexto = None;
        let existentes = self.linhas_de_nomes();
        let mut estado = Estado::default();
        if preencher {
            let mut n = existentes.len() + 1;
            while existentes.iter().any(|e| e.nome.to_uppercase() == format!("Intervalo{n}").to_uppercase()) {
                n += 1;
            }
            estado.nome = format!("Intervalo{n}");
            estado.modo = Modo::Novo;
            estado.refere = texto_refere(&self.area_selecionada_na_aba());
        } else if let Some(primeiro) = existentes.first() {
            estado.selecionado = Some(primeiro.chave.clone());
            estado.refere = self.refere_da_linha(primeiro);
        }
        self.nomes_modal = Some(estado);
        Task::none()
    }

    /// A seleção com o nome da aba atual (o que um nome novo aponta).
    fn area_selecionada_na_aba(&self) -> Rect {
        let mut area = self.area_selecionada();
        area.sheet = self.workbook.aba(self.folha).map(|a| a.sheet.name.clone());
        area
    }

    /// A lista do Escopo: "Pasta de trabalho" e as abas. Editando, só mostra o escopo do nome (não muda).
    fn escolha_de_escopo<'a>(&self, estado: &Estado, novo: bool) -> Element<'a, Msg> {
        let pasta = tr!("Pasta de trabalho").to_owned();
        if !novo {
            let atual = match &estado.modo { Modo::Editar(o) => self.linha_de_nome(o).and_then(|l| l.escopo), _ => None };
            return text(atual.unwrap_or(pasta)).size(13).into();
        }
        // Uma aba chamada "Pasta de trabalho" vai entre aspas, para não se confundir com a pasta.
        let abas = self.workbook.nomes().into_iter().map(|n| if n == pasta { format!("'{n}'") } else { n });
        let opcoes: Vec<String> = std::iter::once(pasta.clone()).chain(abas).collect();
        let atual = estado.escopo.as_ref().and_then(|a| self.workbook.indice_por_nome(a)).map_or(0, |i| i + 1);
        let lista = opcoes.clone();
        crate::compat::pick_list_vec(opcoes, lista.get(atual).cloned(), move |r: String| {
            Msg::NomesEvento(Evento::Escopo(lista.iter().position(|x| *x == r).unwrap_or(0)))
        }).width(Length::FILL).into()
    }

    /// Grava o nome: intervalo, ou fórmula (`=0,1`, `=Vendas!$A$1*2`); com `escopo`, só naquela aba.
    fn gravar_nome(&mut self, nome: &str, alvo: &Result<Rect, String>, escopo: Option<&str>) {
        match (alvo, escopo) {
            (Ok(area), None) => self.workbook.definir_nome(nome, area.clone()),
            (Ok(area), Some(aba)) => self.workbook.definir_nome_formula(nome, &texto_refere(area), Some(aba)),
            (Err(formula), _) => self.workbook.definir_nome_formula(nome, formula, escopo),
        }
        self.alterado = true;
    }

    /// Cria (modo Novo ou Lista) ou regrava (modo Editar) um nome. Sem
    /// "Refere-se a" escrito, usa a seleção. Não edita células.
    pub(super) fn criar_nome(&mut self) -> Task<Msg> {
        let Some((nome, refere, modo)) = self.nomes_modal.as_ref()
            .map(|e| (e.nome.trim().to_owned(), e.refere.clone(), e.modo.clone())) else {
            return Task::none();
        };
        let erro = |app: &mut App, msg: &str| {
            if let Some(e) = &mut app.nomes_modal { e.erro = Some(msg.into()); }
            Task::none()
        };
        if let Err(e) = validar_nome(&nome) { return erro(self, e); }
        let original = match &modo { Modo::Editar(o) => self.linha_de_nome(o), _ => None };
        // Editar mantém o escopo (o Excel também não deixa trocar); o nome novo usa o escolhido no formulário.
        let escopo = match &original {
            Some(o) => o.escopo.clone(),
            None => self.nomes_modal.as_ref().and_then(|e| e.escopo.clone())
                .filter(|aba| self.workbook.indice_por_nome(aba).is_some()),
        };
        // Repetido: o mesmo nome no mesmo escopo (o da pasta e o de uma aba podem ter o mesmo nome).
        let duplicado = self.linhas_de_nomes().iter().any(|e| {
            e.nome.to_uppercase() == nome.to_uppercase() && e.escopo == escopo
                && original.as_ref().is_none_or(|o| o.chave != e.chave)
        });
        if duplicado {
            return erro(self, tr!("Esse nome já existe. Escolha outro."));
        }
        let alvo = if matches!(modo, Modo::Lista) || refere.trim().is_empty() {
            Ok(self.area_selecionada_na_aba())
        } else {
            match self.ler_refere_ou_formula(&refere) {
                Ok(a) => a,
                Err(e) => return erro(self, e),
            }
        };
        if let Some(o) = &original {
            self.apagar_linha(o);
        }
        self.gravar_nome(&nome, &alvo, escopo.as_deref());
        self.status = if original.is_some() {
            trf!("Nome \"{nome}\" alterado.")
        } else {
            trf!("Intervalo \"{nome}\" criado.")
        };
        self.endereco = nome.clone();
        // Volta para a tabela com o nome criado escolhido.
        let chave = chave_do_nome(&nome, escopo.as_deref());
        let refere = self.linha_de_nome(&chave).map(|l| self.refere_da_linha(&l)).unwrap_or_default();
        if let Some(e) = &mut self.nomes_modal {
            e.modo = Modo::Lista;
            e.erro = None;
            e.selecionado = Some(chave);
            e.refere = refere;
            e.nome.clear();
        }
        Task::none()
    }

    fn apagar_linha(&mut self, l: &LinhaNome) -> bool {
        match &l.escopo {
            Some(aba) => self.workbook.remover_nome_formula(&l.nome, Some(aba)),
            None => self.workbook.remover_nome(&l.nome),
        }
    }

    /// Exclui pela chave da tabela (`Nome` ou `Aba!Nome`).
    pub(super) fn excluir_nome(&mut self, chave: &str) -> Task<Msg> {
        let Some(l) = self.linha_de_nome(chave) else { return Task::none() };
        if self.apagar_linha(&l) {
            let nome = l.nome.clone();
            self.alterado = true;
            self.status = trf!("Intervalo \"{nome}\" removido.");
            if self.endereco.to_uppercase() == nome.to_uppercase() {
                self.endereco = cell_name(self.linha, self.coluna);
            }
            if let Some(e) = &mut self.nomes_modal {
                if e.selecionado.as_ref().is_some_and(|s| s.eq_ignore_ascii_case(chave)) {
                    e.selecionado = None;
                    e.refere.clear();
                }
            }
        }
        Task::none()
    }

    /// Vai para o intervalo do nome (chave da tabela, ou o nome digitado na caixa de nome: o da aba
    /// atual vence o da pasta). Nome de valor/fórmula não tem lugar: o status diz.
    pub(super) fn ir_para_nome(&mut self, chave: &str) -> Task<Msg> {
        let linhas = self.linhas_de_nomes();
        let aba_atual = self.workbook.aba(self.folha).map(|a| a.sheet.name.clone());
        let alvo = linhas.iter().find(|l| l.chave.eq_ignore_ascii_case(chave))
            .or_else(|| linhas.iter().find(|l| l.nome.eq_ignore_ascii_case(chave) && l.escopo.is_some()
                && l.escopo.as_deref().map(str::to_lowercase) == aba_atual.as_deref().map(str::to_lowercase)))
            .or_else(|| linhas.iter().find(|l| l.nome.eq_ignore_ascii_case(chave) && l.escopo.is_none()));
        let Some(alvo) = alvo.cloned() else {
            let nome = chave;
            self.status = trf!("O intervalo \"{nome}\" não existe.");
            return Task::none();
        };
        let Some(area) = alvo.area.clone() else {
            self.status = trf!("O nome \"{}\" é um valor ou fórmula, não um intervalo: veja no Gerenciador de nomes.", alvo.nome);
            return Task::none();
        };
        let nome = alvo.nome.clone();
        let folha = if let Some(nome_aba) = area.sheet.as_deref() {
            let Some(i) = self.workbook.nomes().iter()
                .position(|s| s.to_lowercase() == nome_aba.to_lowercase()) else {
                self.status = trf!("A aba do intervalo \"{nome}\" não existe.");
                return Task::none();
            };
            i
        } else {
            self.folha
        };
        if area.r1 >= MAX_ROWS || area.c1 >= MAX_COLS {
            self.status = tr!("Esse intervalo excede os limites da planilha.").into();
            return Task::none();
        }
        if !self.confirmar() { return Task::none(); }
        self.folha = folha;
        self.nome_aba = self.workbook.nomes()[folha].clone();
        let area = area.normalized();
        self.trocar_selecao(area.r0, area.c0);
        self.fim_selecao = (area.r1, area.c1);
        self.endereco = nome.clone();
        self.nomes_modal = None;
        self.status = trf!("Selecionado: {}", nome);
        Task::none()
    }

    pub(super) fn evento_nomes(&mut self, evento: Evento) -> Task<Msg> {
        let linhas = self.linhas_de_nomes();
        let achar = |n: &str| linhas.iter().find(|e| e.chave.eq_ignore_ascii_case(n)).cloned();
        let refere_de = |app: &App, l: &LinhaNome| app.refere_da_linha(l);
        let selecao = self.area_selecionada_na_aba();
        let Some(estado) = &self.nomes_modal else { return Task::none() };
        let selecionado = estado.selecionado.clone();
        match evento {
            Evento::Selecionar(chave) => {
                if let Some(n) = achar(&chave) {
                    let refere = refere_de(self, &n);
                    let Some(e) = &mut self.nomes_modal else { return Task::none() };
                    e.refere = refere;
                    e.selecionado = Some(n.chave);
                    e.modo = Modo::Lista;
                    e.erro = None;
                }
            }
            Evento::Escopo(i) => {
                let aba = i.checked_sub(1).and_then(|i| self.workbook.nomes().get(i).cloned());
                if let Some(e) = &mut self.nomes_modal { e.escopo = aba; }
            }
            Evento::Novo => {
                let mut n = linhas.len() + 1;
                while linhas.iter().any(|x| x.nome.to_uppercase() == format!("INTERVALO{n}")) { n += 1; }
                let Some(e) = &mut self.nomes_modal else { return Task::none() };
                e.nome = format!("Intervalo{n}");
                e.refere = texto_refere(&selecao);
                e.modo = Modo::Novo;
                e.escopo = None;
                e.erro = None;
            }
            Evento::Editar => {
                if let Some(n) = selecionado.as_deref().and_then(achar) {
                    let refere = refere_de(self, &n);
                    let Some(e) = &mut self.nomes_modal else { return Task::none() };
                    e.nome = n.nome.clone();
                    e.refere = refere;
                    e.modo = Modo::Editar(n.chave);
                    e.erro = None;
                }
            }
            Evento::Excluir => {
                let Some(e) = &mut self.nomes_modal else { return Task::none() };
                if let Some(n) = selecionado { e.modo = Modo::Excluir(n); }
            }
            Evento::ExcluirConfirmar => {
                let Some(e) = &mut self.nomes_modal else { return Task::none() };
                if let Modo::Excluir(chave) = std::mem::take(&mut e.modo) {
                    let _ = self.excluir_nome(&chave);
                    let primeiro = self.linhas_de_nomes().into_iter().next();
                    let refere = primeiro.as_ref().map(|p| self.refere_da_linha(p));
                    if let (Some(e), Some(p), Some(r)) = (&mut self.nomes_modal, primeiro, refere) {
                        e.selecionado = Some(p.chave);
                        e.refere = r;
                    }
                }
            }
            Evento::Refere(texto) => {
                let Some(e) = &mut self.nomes_modal else { return Task::none() };
                e.refere = texto;
                e.erro = None;
            }
            Evento::Aplicar => {
                let Some(linha) = selecionado.as_deref().and_then(achar) else { return Task::none() };
                let refere = estado.refere.clone();
                match self.ler_refere_ou_formula(&refere) {
                    Ok(alvo) => {
                        // Trocar intervalo por fórmula (ou o contrário) num nome da pasta: o antigo sai.
                        self.apagar_linha(&linha);
                        self.gravar_nome(&linha.nome, &alvo, linha.escopo.as_deref());
                        let novo = self.linha_de_nome(&linha.chave);
                        let mostrado = novo.as_ref().map(|l| self.refere_da_linha(l)).unwrap_or_default();
                        let grafia = &linha.nome;
                        self.status = trf!("Nome \"{grafia}\" agora se refere a {}.", mostrado);
                        if let Some(e) = &mut self.nomes_modal { e.refere = mostrado; }
                    }
                    Err(msg) => {
                        if let Some(e) = &mut self.nomes_modal { e.erro = Some(msg.into()); }
                    }
                }
            }
            Evento::Voltar => {
                let refere = selecionado.as_deref().and_then(achar).map(|n| refere_de(self, &n));
                let Some(e) = &mut self.nomes_modal else { return Task::none() };
                e.modo = Modo::Lista;
                e.erro = None;
                e.refere = refere.unwrap_or_default();
            }
        }
        Task::none()
    }

    /// Um valor como a grade mostra na língua da interface (vírgula decimal em português, ponto em inglês).
    fn valor_na_lingua(&self, v: feathersheet_core::Value) -> String {
        match v {
            feathersheet_core::Value::Number(n) => {
                let texto = feathersheet_core::celulas::values::to_text(&feathersheet_core::Value::Number(n));
                texto.replace('.', &self.regionalizacao().decimal_separator.to_string())
            }
            feathersheet_core::Value::Bool(b) => feathersheet_core::celulas::locale::texto_logico(b),
            feathersheet_core::Value::Error(e) => e.code.na_lingua(),
            v => feathersheet_core::celulas::values::to_text(&v),
        }
    }

    /// A coluna Valor de uma linha: o do intervalo ou o da fórmula, calculado na aba do escopo (ou na atual).
    fn valor_da_linha(&self, l: &LinhaNome) -> String {
        if let (None, Some(area)) = (&l.formula, &l.area) { return self.valor_do_nome(area); }
        let aba = l.escopo.as_deref().and_then(|a| self.workbook.indice_por_nome(a)).unwrap_or(self.folha);
        let formula = self.workbook.nomes_formula().into_iter()
            .find(|n| n.nome.eq_ignore_ascii_case(&l.nome) && n.aba == l.escopo);
        match formula.as_ref().and_then(|n| n.arvore()) {
            Some(no) => self.valor_na_lingua(self.workbook.avaliar_em(aba, 0, 0, no)),
            None => "#NOME?".into(),
        }
    }

    /// O que a coluna Valor mostra: o valor da célula, ou `{a; b; …}`.
    fn valor_do_nome(&self, area: &Rect) -> String {
        let Some(aba) = area.sheet.as_deref().and_then(|a| self.workbook.indice_por_nome(a)) else {
            return "#REF!".into();
        };
        let n = area.normalized();
        let texto = |r, c| self.valor_na_lingua(self.workbook.valor(aba, r, c));
        if n.r0 == n.r1 && n.c0 == n.c1 {
            return texto(n.r0, n.c0);
        }
        let ultima = self.workbook.aba(aba).map(|a| a.sheet.used_rect().r1).unwrap_or(0);
        let mut itens = Vec::new();
        'fora: for r in n.r0..=n.r1.min(ultima) {
            for c in n.c0..=n.c1.min(n.c0 + 20) {
                itens.push(texto(r, c));
                if itens.len() == 4 { break 'fora; }
            }
        }
        let total = u64::from(n.r1 - n.r0 + 1) * u64::from(n.c1 - n.c0 + 1);
        let mais = if (itens.len() as u64) < total { "; …" } else { "" };
        format!("{{{}{mais}}}", itens.join("; "))
    }

    fn tabela_nomes(&self, estado: &Estado) -> Element<'_, Msg> {
        let nomes = self.linhas_de_nomes();
        if nomes.is_empty() {
            return dlg::aviso_vazio(tr!("Nenhum nome definido. Use Novo para dar um nome à seleção atual."));
        }
        let semibold = pelicano::Font { weight: 600, ..Default::default() };
        let cabecalho = |t: &str| text(t.to_uppercase()).size(10.5).font(semibold).color(COR_TEXTO_MUTED());
        let titulo = container(row![
            cabecalho(tr!("Nome")).width(Length::Fill(3)),
            cabecalho(tr!("Valor")).width(Length::Fill(2)),
            cabecalho(tr!("Refere-se a")).width(Length::Fill(4)),
            cabecalho(tr!("Escopo")).width(Length::Fill(3)),
        ].spacing(10)).padding([4, 10]);
        let mut linhas = pelicano::column(Vec::<pelicano::Element<_>>::new()).spacing(1);
        for n in nomes {
            let ativo = estado.selecionado.as_ref().is_some_and(|s| s.eq_ignore_ascii_case(&n.chave));
            let escopo = n.escopo.clone().unwrap_or_else(|| tr!("Pasta de trabalho").to_owned());
            let linha = row![
                text(cortar(&n.nome, 22)).size(12.5).width(Length::Fill(3)),
                text(cortar(&self.valor_da_linha(&n), 16)).size(12.5).color(COR_TEXTO_MUTED())
                    .width(Length::Fill(2)),
                text(cortar(&self.refere_da_linha(&n), 30)).size(12.5).width(Length::Fill(4)),
                text(cortar(&escopo, 22)).size(12.5).color(COR_TEXTO_MUTED()).width(Length::Fill(3)),
            ].spacing(10).align(pelicano::Align::Center);
            linhas = linhas.push(
                button(linha).width(Length::FILL).padding([6, 10])
                    .style(estilo_botao_pequeno(ativo))
                    .on_press(Msg::NomesEvento(Evento::Selecionar(n.chave))),
            );
        }
        container(pelicano::column![
            titulo,
            container(Space::new(Length::FILL, Length::Fixed(1.0))).style(estilo_linha),
            container(dlg::rolagem(linhas)).max_height(232.0),
        ].spacing(2))
            .padding(4).width(Length::FILL).style(dlg::estilo_bloco).into()
    }

    fn view_nomes(&self) -> Dialogo<'_, Msg> {
        let estado = self.nomes_modal.as_ref().expect("gerenciador de nomes aberto");
        let tem = estado.selecionado.is_some();
        let acao = |icone: &str, rotulo: &str, msg: Option<Msg>| -> Element<'_, Msg> {
            button(row![feather_ui::icones::icone(icone, 15.0), text(rotulo.to_owned()).size(12.5)]
                .spacing(6).align(pelicano::Align::Center))
                .padding([5, 10]).style(estilo_botao_secundario).on_press_maybe(msg).into()
        };
        let em_lista = matches!(estado.modo, Modo::Lista);
        let barra = row![
            acao("plus", tr!("Novo…"), Some(Msg::NomesEvento(Evento::Novo))),
            acao("edit", tr!("Editar…"), (tem && em_lista).then_some(Msg::NomesEvento(Evento::Editar))),
            acao("trash", tr!("Excluir"), (tem && em_lista).then_some(Msg::NomesEvento(Evento::Excluir))),
        ].spacing(6);

        let rotulo = |t: &str| text(t.to_owned()).size(12).color(COR_TEXTO_MUTED());
        let erro: Option<Element<'_, Msg>> = estado.erro.as_ref().map(|e| {
            row![feather_ui::icones::icone("info-circle", 15.0), text(e.clone()).size(12).color(COR_ERRO())]
                .spacing(6).align(pelicano::Align::Center).into()
        });

        let mut corpo = pelicano::column![barra, self.tabela_nomes(estado)].spacing(10);
        let d = match &estado.modo {
            Modo::Lista => {
                let mut campo = text_input(tr!("Escolha um nome na tabela"), &estado.refere).padding(8).size(13);
                if tem {
                    campo = campo.on_input(|t| Msg::NomesEvento(Evento::Refere(t)))
                        .on_submit(Msg::NomesEvento(Evento::Aplicar));
                }
                corpo = corpo.push(pelicano::column![rotulo(tr!("Refere-se a")), campo].spacing(5));
                if let Some(e) = erro { corpo = corpo.push(e); }
                let atual = estado.selecionado.as_deref().and_then(|s| self.linha_de_nome(s))
                    .map(|n| self.refere_da_linha(&n));
                let mudou = atual.is_some_and(|a| a != estado.refere.trim());
                self.dialogo(tr!("Gerenciador de nomes"), corpo)
                    .rodape_esquerda(dlg::botao_secundario(tr!("Ir para"),
                        estado.selecionado.clone().map(Msg::NomeIrPara)))
                    .primario(tr!("Aplicar"), mudou.then_some(Msg::NomesEvento(Evento::Aplicar)))
            }
            Modo::Novo | Modo::Editar(_) => {
                let novo = matches!(estado.modo, Modo::Novo);
                let form = pelicano::column![
                    dlg::secao(if novo { tr!("Novo nome") } else { tr!("Editar nome") }),
                    rotulo(tr!("Nome")),
                    text_input(tr!("Ex.: receitas_2026"), &estado.nome)
                        .on_input(Msg::NomeAtualizar).on_submit(Msg::NomeCriar).padding(8).size(13),
                    rotulo(tr!("Escopo")),
                    self.escolha_de_escopo(estado, novo),
                    rotulo(tr!("Refere-se a")),
                    text_input("=Planilha1!$A$1:$B$10", &estado.refere)
                        .on_input(|t| Msg::NomesEvento(Evento::Refere(t)))
                        .on_submit(Msg::NomeCriar).padding(8).size(13),
                ].spacing(6);
                corpo = corpo.push(container(form).padding([10, 12]).width(Length::FILL).style(dlg::estilo_bloco));
                if let Some(e) = erro { corpo = corpo.push(e); }
                self.dialogo(tr!("Gerenciador de nomes"), corpo)
                    .secundario(tr!("Cancelar"), Some(Msg::NomesEvento(Evento::Voltar)))
                    .primario(if novo { tr!("Criar nome") } else { tr!("Salvar") }, Some(Msg::NomeCriar))
            }
            Modo::Excluir(nome) => {
                corpo = corpo.push(dlg::aviso("alert-triangle", trf!(
                    "Excluir o nome \"{nome}\"? As fórmulas que o usam passam a mostrar #NOME?.")));
                self.dialogo(tr!("Gerenciador de nomes"), corpo)
                    .secundario(tr!("Cancelar"), Some(Msg::NomesEvento(Evento::Voltar)))
                    .primario(tr!("Excluir nome"), Some(Msg::NomesEvento(Evento::ExcluirConfirmar)))
            }
        };
        d.icone("tag")
            .subtitulo(tr!("Use os nomes em fórmulas, como =SOMA(receitas). \"Refere-se a\" aceita um intervalo ou uma conta, como =0,1."))
            .largura(740.0)
            .ao_fechar(Msg::NomesFechar)
    }

    pub(super) fn sobrepor_nomes<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.nomes_modal.is_none() { return base; }
        self.view_nomes().sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn valida_nomes_como_o_typescript_sem_confundir_enderecos() {
        for nome in ["Receitas", "preços_2026", "_base.2025", "CEP", "Rx", "Custo"] {
            assert!(validar_nome(nome).is_ok(), "{nome}");
        }
        for nome in ["", "A1", "XFD10000000", "A1:B2", "com espaço", "12abc", "a/b", "R", "c", "RC", "R1C1", "r2", "R1C1Nome", "R1x", "C1_"] {
            assert!(validar_nome(nome).is_err(), "{nome}");
        }
    }

    #[test]
    fn caixa_de_nome_mostra_o_nome_da_selecao_e_uma_celula_sem_repetir() {
        let mut app = App::default();
        app.workbook.definir_nome("teste1", Rect::na_aba(16, 6, 16, 6, "Planilha1"));
        app.trocar_selecao(16, 6);
        assert_eq!(app.nome_da_selecao().as_deref(), Some("teste1"));
        app.fim_selecao = (17, 6);
        assert_eq!(app.nome_da_selecao(), None, "G17:G18 não é o nome");
        // #21/#26: Shift+clique de volta na mesma célula escrevia `G17:G17`.
        let _ = app.update(Msg::Estender(16, 6));
        assert_eq!(app.endereco, "G17");
        let _ = app.update(Msg::Estender(18, 7));
        assert_eq!(app.endereco, "G17:H19");
    }

    #[test]
    fn cria_navega_e_remove_sem_alterar_celulas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 1, 2, "123".into());
        app.linha = 1;
        app.coluna = 2;
        app.fim_selecao = (3, 4);
        app.update(Msg::NomesAbrir(false));
        app.update(Msg::NomeAtualizar("Receitas".into()));
        app.update(Msg::NomeCriar);
        assert_eq!(app.workbook.nomeados()[0].area, Rect::na_aba(1, 2, 3, 4, "Planilha1"));
        assert!(app.alterado);
        app.linha = 0;
        app.coluna = 0;
        app.fim_selecao = (0, 0);
        app.update(Msg::IrPara("receitas".into()));
        assert_eq!((app.linha, app.coluna, app.fim_selecao), (1, 2, (3, 4)));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1, 2), "123");
        app.update(Msg::NomeRemover("RECEITAS".into()));
        assert!(app.workbook.nomeados().is_empty());
    }

    /// Escopo escolhido no "Novo nome": o nome vale só naquela aba; o mesmo nome na pasta continua possível.
    #[test]
    fn novo_nome_com_escopo_de_uma_aba() {
        let mut app = App::default();
        app.workbook.inserir_aba("Vendas", 1);
        app.update(Msg::NomesAbrir(false));
        app.update(Msg::NomesEvento(Evento::Novo));
        app.update(Msg::NomeAtualizar("Taxa".into()));
        app.update(Msg::NomesEvento(Evento::Refere("=0,1".into())));
        app.update(Msg::NomesEvento(Evento::Escopo(2)));
        app.update(Msg::NomeCriar);
        let linhas = app.linhas_de_nomes();
        assert!(linhas.iter().any(|l| l.nome == "Taxa" && l.escopo.as_deref() == Some("Vendas")), "{:?}", linhas.iter().map(|l| (&l.nome, &l.escopo)).collect::<Vec<_>>());
        // O mesmo nome na pasta inteira não é duplicado.
        app.update(Msg::NomesEvento(Evento::Novo));
        app.update(Msg::NomeAtualizar("Taxa".into()));
        app.update(Msg::NomesEvento(Evento::Refere("=0,2".into())));
        app.update(Msg::NomeCriar);
        assert!(app.nomes_modal.as_ref().unwrap().erro.is_none());
        assert_eq!(app.linhas_de_nomes().iter().filter(|l| l.nome == "Taxa").count(), 2);
        // Na aba Vendas, =Taxa é 0,1; na outra, 0,2.
        app.workbook.escrever_celula(1, 0, 0, "=Taxa*100".into());
        app.workbook.escrever_celula(0, 0, 0, "=Taxa*100".into());
        assert_eq!(app.workbook.valor(1, 0, 0), feathersheet_core::Value::Number(10.0));
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Number(20.0));
    }

    #[test]
    fn gerenciador_recusa_nome_duplicado_e_referencia_em_outra_aba() {
        let mut app = App::default();
        app.update(Msg::NomesAbrir(false));
        app.update(Msg::NomeAtualizar("Base".into()));
        app.update(Msg::NomeCriar);
        app.update(Msg::NomesAbrir(false));
        app.update(Msg::NomeAtualizar("base".into()));
        app.update(Msg::NomeCriar);
        assert!(app.nomes_modal.as_ref().unwrap().erro.is_some());
        assert_eq!(app.workbook.nomeados().len(), 1);
        app.update(Msg::NomesFechar);
        app.workbook.definir_nome("Invalido", Rect::na_aba(0,0,0,0,"Excluida"));
        app.update(Msg::IrPara("Invalido".into()));
        assert!(app.status.contains("não existe"));
    }

    #[test]
    fn refere_se_a_ida_e_volta() {
        let wb = Workbook::new();
        let aba = wb.nomes()[0].clone();
        let r = Rect::na_aba(0, 0, 9, 1, aba.clone());
        assert_eq!(texto_refere(&r), format!("={aba}!$A$1:$B$10"));
        assert_eq!(ler_refere(&wb, &texto_refere(&r), &aba).unwrap(), r);
        assert_eq!(ler_refere(&wb, "B7", &aba).unwrap(), Rect::na_aba(6, 1, 6, 1, aba.clone()));
        assert_eq!(texto_refere(&Rect::na_aba(6, 1, 6, 1, aba.clone())), format!("={aba}!$B$7"));
        assert!(ler_refere(&wb, "=Outra!A1", &aba).is_err());
        assert!(ler_refere(&wb, "=", &aba).is_err());
        assert!(ler_refere(&wb, "zz", &aba).is_err());
    }

    #[test]
    fn novo_editar_aplicar_e_excluir_com_confirmacao() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "5".into());
        app.update(Msg::NomesAbrir(true));
        assert_eq!(app.nomes_modal.as_ref().unwrap().modo, Modo::Novo);
        assert_eq!(app.nomes_modal.as_ref().unwrap().refere, "=Planilha1!$A$1");
        app.update(Msg::NomeAtualizar("Taxa".into()));
        app.update(Msg::NomeCriar);
        let e = app.nomes_modal.as_ref().unwrap();
        assert_eq!((e.modo.clone(), e.selecionado.as_deref()), (Modo::Lista, Some("Taxa")));
        assert_eq!(app.valor_do_nome(&app.workbook.nomeados()[0].area), "5");
        app.workbook.escrever_celula(0, 0, 0, "0,12".into());
        assert_eq!(app.valor_do_nome(&app.workbook.nomeados()[0].area), "0,12");
        let _: Element<'_, Msg> = app.view_nomes().painel();

        // "Refere-se a" editado na lista.
        app.update(Msg::NomesEvento(Evento::Refere("=Planilha1!$A$1:$A$3".into())));
        app.update(Msg::NomesEvento(Evento::Aplicar));
        assert_eq!(app.workbook.nomeados()[0].area, Rect::na_aba(0, 0, 2, 0, "Planilha1"));

        // Editar renomeia.
        app.update(Msg::NomesEvento(Evento::Editar));
        app.update(Msg::NomeAtualizar("Juros".into()));
        let _: Element<'_, Msg> = app.view_nomes().painel();
        app.update(Msg::NomeCriar);
        let nomes: Vec<String> = app.workbook.nomeados().into_iter().map(|n| n.nome).collect();
        assert_eq!(nomes, vec!["Juros".to_string()]);

        // Excluir pede confirmação.
        app.update(Msg::NomesEvento(Evento::Excluir));
        assert_eq!(app.workbook.nomeados().len(), 1);
        let _: Element<'_, Msg> = app.view_nomes().painel();
        app.update(Msg::NomesEvento(Evento::ExcluirConfirmar));
        assert!(app.workbook.nomeados().is_empty());
        let _: Element<'_, Msg> = app.view_nomes().painel();
    }

    /// "Refere-se a" com uma conta vira nome de valor (`Taxa` = `=0,1`), que calcula nas células;
    /// a caixa de nome digitando "Taxa" vai para o nome, sem trocá-lo pela seleção.
    #[test]
    fn refere_se_a_com_conta_cria_nome_de_valor() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "1000".into());
        app.update(Msg::NomesAbrir(true));
        app.update(Msg::NomeAtualizar("Taxa".into()));
        app.update(Msg::NomesEvento(Evento::Refere("=0,1".into())));
        app.update(Msg::NomeCriar);
        assert!(app.nomes_modal.as_ref().unwrap().erro.is_none(), "{:?}", app.nomes_modal.as_ref().unwrap().erro);
        let n = app.workbook.nomes_formula();
        assert_eq!((n[0].nome.as_str(), n[0].formula.as_str(), n[0].aba.as_deref()), ("Taxa", "=0,1", None));
        assert!(app.workbook.nomeados().is_empty());
        let linha = app.linha_de_nome("Taxa").unwrap();
        assert_eq!(app.valor_da_linha(&linha), "0,1");
        let _: Element<'_, Msg> = app.view_nomes().painel();
        app.workbook.escrever_celula(0, 0, 1, "=A1*Taxa".into());
        assert_eq!(app.workbook.valor(0, 0, 1), feathersheet_core::Value::Number(100.0));
        // Conta inválida é recusada com o motivo.
        app.update(Msg::NomesEvento(Evento::Refere("=0,1+".into())));
        app.update(Msg::NomesEvento(Evento::Aplicar));
        assert!(app.nomes_modal.as_ref().unwrap().erro.is_some());
        app.update(Msg::NomesFechar);
        app.update(Msg::IrPara("Taxa".into()));
        assert_eq!(app.workbook.nomes_formula().len(), 1, "a caixa de nome não troca a fórmula por intervalo");
        assert!(app.workbook.nomeados().is_empty());
        assert!(app.status.contains("valor ou fórmula"), "{}", app.status);
    }

    /// Em inglês a conta aparece e é digitada com ponto decimal; o motor guarda com vírgula.
    #[test]
    fn nome_de_valor_em_ingles_mostra_e_le_ponto_decimal() {
        let mut app = App::default();
        app.config.idioma = "en-US".into();
        app.workbook.definir_nome_formula("Taxa", "=0,1", None);
        app.workbook.escrever_celula(0, 0, 0, "0,12".into());
        app.workbook.definir_nome("Celula", Rect::na_aba(0, 0, 0, 0, "Planilha1"));
        let taxa = app.linha_de_nome("Taxa").unwrap();
        assert_eq!(app.refere_da_linha(&taxa), "=0.1");
        assert_eq!(app.valor_da_linha(&taxa), "0.1");
        assert_eq!(app.valor_da_linha(&app.linha_de_nome("Celula").unwrap()), "0.12", "a coluna Valor segue a língua");
        app.update(Msg::NomesAbrir(false));
        app.update(Msg::NomesEvento(Evento::Selecionar("Taxa".into())));
        app.update(Msg::NomesEvento(Evento::Refere("=0.25".into())));
        app.update(Msg::NomesEvento(Evento::Aplicar));
        // Guardado como digitado (como nas células); o que importa é o valor e o que cada língua mostra.
        app.workbook.escrever_celula(0, 1, 0, "=Taxa*4".into());
        assert_eq!(app.workbook.valor(0, 1, 0), feathersheet_core::Value::Number(1.0));
    }

    /// Nome de uma aba só: aparece com a aba em Escopo, e mudar "Refere-se a" não o leva para a pasta.
    #[test]
    fn nome_de_aba_mantem_o_escopo_ao_aplicar_e_excluir() {
        let mut app = App::default();
        let plan2 = app.workbook.criar_aba("Plan2");
        app.workbook.definir_nome_formula("Meta", "=10", None);
        app.workbook.definir_nome_formula("Meta", "=20", Some("Plan2"));
        app.update(Msg::NomesAbrir(false));
        assert_eq!(app.linhas_de_nomes().len(), 2);
        app.update(Msg::NomesEvento(Evento::Selecionar("Plan2!Meta".into())));
        app.update(Msg::NomesEvento(Evento::Refere("=30".into())));
        app.update(Msg::NomesEvento(Evento::Aplicar));
        let _: Element<'_, Msg> = app.view_nomes().painel();
        app.workbook.escrever_celula(plan2, 0, 0, "=Meta".into());
        app.workbook.escrever_celula(0, 0, 0, "=Meta".into());
        assert_eq!(app.workbook.valor(plan2, 0, 0), feathersheet_core::Value::Number(30.0));
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Number(10.0));
        app.update(Msg::NomesEvento(Evento::Excluir));
        app.update(Msg::NomesEvento(Evento::ExcluirConfirmar));
        let n = app.workbook.nomes_formula();
        assert_eq!((n.len(), n[0].aba.as_deref()), (1, None), "só o da aba saiu: {n:?}");
    }
}

