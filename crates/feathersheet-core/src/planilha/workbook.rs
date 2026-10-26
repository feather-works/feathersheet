//! A pasta de trabalho — as abas, a versão de cada uma e o que sujou.
//!
//! É a peça que faltava para o motor sair do papel. Até 22/09/2026 o Rust
//! sabia tudo (tokenizar, avaliar, guardar em coluna, excluir coluna) e
//! **não guardava nada**: todo comando do `src-tauri` era sem estado — o
//! JavaScript mandava os dados, o Rust calculava, devolvia e esquecia. Por
//! isso a planilha inteira morava no WebKit, e era ele que estourava.
//!
//! A partir daqui a planilha mora **aqui**. O TypeScript manda a intenção
//! ("escreva isto nesta célula", "exclua a coluna D") e recebe de volta a
//! versão nova e os retângulos que sujaram; o conteúdo ele pede depois, em
//! [`crate::planilha::frame`], e só o pedaço que está na tela.
//!
//! ## A versão, e por que ela é por aba
//!
//! Todo comando que muda alguma coisa sobe a versão da aba mexida. O
//! TypeScript guarda a versão do quadro que tem na mão; quando pede o quadro
//! de novo com a mesma versão, a resposta é de oito bytes dizendo "nada
//! mudou". É o caso comum — a pessoa mexeu o mouse, a janela repintou — e é
//! o que impede a ponte de virar o gargalo novo.
//!
//! Por aba, e não da pasta inteira, porque editar a `Plan2` não pode invalidar
//! o quadro de quem está olhando a `Plan1`.
//!
//! ## Os retângulos sujos
//!
//! Quem muda uma célula não sabe o que mais mudou junto: uma fórmula três
//! colunas adiante pode depender dela. Em vez de devolver a lista exata (que
//! exigiria consultar o grafo e pode ser enorme), cada comando devolve
//! **retângulos** — grosseiros de propósito. O TypeScript os usa só para
//! decidir se o que está na tela precisa ser pedido de novo, e pedir a mais é
//! barato: o quadro é pequeno.

use std::collections::HashMap;

use crate::celulas::refs::{key_col, key_row, CellKey, Rect};
use crate::planilha::sheet::{Hospedeiro, Sheet};
use crate::celulas::values::Value;

/// O que um comando que mudou alguma coisa devolve.
#[derive(Debug, Clone, PartialEq)]
pub struct Mudanca {
    /// A versão nova da aba mexida.
    pub versao: u32,
    /// Onde mexeu, grosseiramente. Ver a nota do topo.
    pub sujos: Vec<Rect>,
}

/// Uma aba viva, com a versão do conteúdo dela.
pub struct AbaViva {
    pub sheet: Sheet,
    /// Sobe a cada mudança. Começa em 1: zero fica reservado para "nunca vi
    /// esta aba", que é o que o TypeScript manda no primeiro pedido.
    pub versao: u32,
    /// Quem é esta aba para o TypeScript. O índice muda quando se cria,
    /// apaga ou move uma aba; o `id`, não — é por ele que os comandos a
    /// acham, senão uma aba apagada faria o TypeScript escrever na vizinha.
    pub id: u32,
}

/// Um intervalo nomeado, com a grafia que a pessoa escolheu.
#[derive(Debug, Clone, PartialEq)]
pub struct Nomeado {
    pub nome: String,
    pub area: Rect,
}

/// Propriedades do arquivo (no Excel, Arquivo → Informações): quem criou,
/// quem alterou por último e a organização. A interface preenche com os
/// "Seus dados" das Configurações; nada sai do computador.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Propriedades {
    pub autor: String,
    pub modificado_por: String,
    pub organizacao: String,
}

impl Propriedades {
    pub fn vazia(&self) -> bool {
        self.autor.is_empty() && self.modificado_por.is_empty() && self.organizacao.is_empty()
    }
}

/// A pasta inteira, viva na memória do Rust.
pub struct Workbook {
    abas: Vec<AbaViva>,
    proximo_id: u32,
    /// Os intervalos nomeados, pela chave em maiúscula (como o TypeScript).
    nomeados: HashMap<String, Nomeado>,
    /// As outras pastas que as fórmulas citam (`'[plan_b.plz]Aba'!A1`),
    /// só com valores. Ver [`crate::planilha::vinculo`].
    externos: crate::planilha::vinculo::Externos,
    /// As funções criadas por macro (em maiúscula). A interface as executa;
    /// o motor só sabe que o nome existe.
    pub funcoes_externas: std::collections::HashSet<String>,
    /// Desfazer/refazer das operações que mexem na lista de abas (hoje,
    /// duplicar). O desfazer de células mora em cada [`Sheet`].
    hist_abas: HistoricoAbas,
    /// Cálculo manual (Fórmulas → Opções de cálculo). Ver
    /// [`crate::planilha::calculo_manual`].
    pub(crate) manual: bool,
    /// Mudanças de aba cujo efeito nas fórmulas de outras abas espera o F9.
    pub(crate) externas_adiadas: Vec<(usize, Option<Vec<Rect>>)>,
    /// Estilos de célula criados pela pessoa (Estilos de célula ▾ → Novo
    /// estilo), como JSON opaco: quem lê e escreve é a interface, como os
    /// outros metadados (o motor não lê JSON).
    pub estilos_celula: Option<String>,
    /// Autor, última alteração e organização (gravados no .fshz e no .xlsx).
    pub propriedades: Propriedades,
    /// As macros desta pasta (catálogo JSON opaco, como os estilos de
    /// célula): cada arquivo leva as suas; o motor não as lê.
    pub macros: Option<String>,
}

/// Uma duplicação de aba, desfazível. `aba` é `Some` enquanto a cópia está
/// fora da pasta (desfeita) — guardá-la inteira preserva o `id` e o
/// histórico dela para o refazer.
struct OpDuplicar {
    id: u32,
    em: usize,
    aba: Option<AbaViva>,
    /// `(id, versão)` de todas as abas logo depois da operação (ou do
    /// desfazer/refazer). Se mudou algo desde então, a edição mais recente
    /// não é esta duplicação e o Ctrl+Z não deve tocá-la.
    assinatura: Vec<(u32, u32)>,
}

#[derive(Default)]
struct HistoricoAbas {
    desfazer: Vec<OpDuplicar>,
    refazer: Vec<OpDuplicar>,
}

impl Default for Workbook {
    fn default() -> Self {
        Self::new()
    }
}

impl Workbook {
    /// Uma pasta nova, com uma aba vazia — como o aplicativo abre.
    pub fn new() -> Self {
        let mut w = Workbook::vazia();
        w.criar_aba("Planilha1");
        w
    }

    /// Uma pasta sem aba nenhuma, para quem vai carregar de um arquivo.
    pub fn vazia() -> Self {
        Workbook {
            abas: Vec::new(),
            proximo_id: 1,
            nomeados: HashMap::new(),
            externos: crate::planilha::vinculo::Externos::default(),
            funcoes_externas: std::collections::HashSet::new(),
            manual: false,
            externas_adiadas: Vec::new(),
            hist_abas: HistoricoAbas::default(),
            estilos_celula: None,
            propriedades: Propriedades::default(),
            macros: None,
        }
    }

    /// Troca a lista de funções criadas por macro e recalcula as fórmulas
    /// que citam um nome que entrou ou saiu.
    pub fn definir_funcoes_externas(&mut self, nomes: impl IntoIterator<Item = String>) {
        let novas: std::collections::HashSet<String> = nomes.into_iter().map(|n| n.to_uppercase()).collect();
        let mudaram: Vec<String> = novas.symmetric_difference(&self.funcoes_externas).cloned().collect();
        self.funcoes_externas = novas;
        if mudaram.is_empty() {
            return;
        }
        for i in 0..self.abas.len() {
            self.editar(i, |s| {
                for nome in &mudaram {
                    s.sujar_as_que_citam(nome);
                }
            });
        }
    }

    pub fn len(&self) -> usize {
        self.abas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.abas.is_empty()
    }

    /// Os nomes das abas, na ordem.
    pub fn nomes(&self) -> Vec<String> {
        self.abas.iter().map(|a| a.sheet.name.clone()).collect()
    }

    /// Acrescenta uma aba e devolve o índice dela.
    pub fn criar_aba(&mut self, nome: impl Into<String>) -> usize {
        let n = self.abas.len();
        self.inserir_aba(nome, n)
    }

    /// Acrescenta uma aba na posição `em` e devolve o índice dela.
    pub fn inserir_aba(&mut self, nome: impl Into<String>, em: usize) -> usize {
        let id = self.proximo_id;
        self.proximo_id += 1;
        let em = em.min(self.abas.len());
        self.abas.insert(em, AbaViva { sheet: Sheet::new(nome), versao: 1, id });
        self.abas[em].sheet.manual.set(self.manual);
        self.hist_abas = HistoricoAbas::default();
        em
    }

    /// O índice da aba com este `id`.
    pub fn indice_do_id(&self, id: u32) -> Option<usize> {
        self.abas.iter().position(|a| a.id == id)
    }

    /// Os `id` das abas, na ordem.
    pub fn ids(&self) -> Vec<u32> {
        self.abas.iter().map(|a| a.id).collect()
    }

    /// Índices de abas visíveis; a posição física nunca muda ao ocultar.
    pub fn abas_visiveis(&self) -> impl Iterator<Item = usize> + '_ {
        self.abas.iter().enumerate()
            .filter_map(|(i, aba)| (!aba.sheet.oculta).then_some(i))
    }

    /// Abas ocultas, sem carregar ou copiar as células.
    pub fn abas_ocultas(&self) -> Vec<usize> {
        self.abas.iter().enumerate()
            .filter_map(|(i, aba)| aba.sheet.oculta.then_some(i)).collect()
    }

    /// Oculta uma aba, exigindo que pelo menos uma permaneça visível.
    /// Devolve uma aba visível segura para seleção na GUI.
    pub fn ocultar_aba(&mut self, indice: usize) -> Result<usize, &'static str> {
        if indice >= self.abas.len() {
            return Err("A aba não existe");
        }
        if self.abas[indice].sheet.oculta {
            return Err("A aba já está oculta");
        }
        if self.abas_visiveis().count() <= 1 {
            return Err("A pasta precisa ter pelo menos uma aba visível");
        }
        self.abas[indice].sheet.oculta = true;
        self.abas[indice].versao = self.abas[indice].versao.wrapping_add(1);
        self.hist_abas = HistoricoAbas::default();
        Ok(self.abas_visiveis().next().expect("pelo menos uma visível"))
    }

    pub fn reexibir_aba(&mut self, indice: usize) -> bool {
        let Some(aba) = self.abas.get_mut(indice) else { return false };
        if !aba.sheet.oculta { return false; }
        aba.sheet.oculta = false;
        aba.versao = aba.versao.wrapping_add(1);
        self.hist_abas = HistoricoAbas::default();
        true
    }

    /// Tira a aba. Recusa a última: uma pasta sempre tem uma aba.
    pub fn remover_aba(&mut self, indice: usize) -> bool {
        if self.abas.len() <= 1 || indice >= self.abas.len() {
            return false;
        }
        if !self.abas[indice].sheet.oculta && self.abas_visiveis().count() <= 1 {
            return false;
        }
        self.abas.remove(indice);
        self.hist_abas = HistoricoAbas::default();
        self.depois_de_mudar(usize::MAX);
        true
    }

    /// Muda a aba de lugar.
    pub fn mover_aba(&mut self, de: usize, para: usize) {
        if de == para || de >= self.abas.len() {
            return;
        }
        let aba = self.abas.remove(de);
        let para = para.min(self.abas.len());
        self.abas.insert(para, aba);
        self.abas[para].sheet.manual.set(self.manual);
        self.hist_abas = HistoricoAbas::default();
    }

    // ------------------------------------------------------ duplicar aba

    /// O nome da cópia: `Base (2)`, `Base (3)`… o primeiro livre. Duplicar
    /// `Plan (2)` dá `Plan (3)`, não `Plan (2) (2)`.
    pub fn nome_da_copia(&self, base: &str) -> String {
        let mut raiz = base.trim();
        if let Some(ab) = raiz.rfind(" (") {
            let resto = &raiz[ab + 2..];
            if let Some(num) = resto.strip_suffix(')') {
                if !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) && ab > 0 {
                    raiz = raiz[..ab].trim_end();
                }
            }
        }
        let mut n = 2;
        loop {
            let candidato = format!("{raiz} ({n})");
            let existe = self.abas.iter()
                .any(|a| a.sheet.name.to_lowercase() == candidato.to_lowercase());
            if !existe {
                return candidato;
            }
            n += 1;
        }
    }

    /// Duplica a aba `indice`, logo depois dela, e devolve o índice da cópia
    /// (a que deve ficar ativa). A cópia é uma `Sheet` nova com tudo da
    /// original; as outras abas não são tocadas — fórmulas que citam a
    /// original continuam citando a original. Desfazível por
    /// [`Self::desfazer_aba`].
    pub fn duplicar_aba(&mut self, indice: usize) -> Option<usize> {
        let origem = self.abas.get(indice)?;
        let nome = self.nome_da_copia(&origem.sheet.name);
        // Nome de tabela é global na pasta: a cópia recebe `TabelaN` livres.
        let mut usados: Vec<String> = self.nomeados.values().map(|n| n.nome.clone()).collect();
        for a in &self.abas {
            usados.extend(a.sheet.nomes_das_tabelas());
        }
        let mut renomes: Vec<(String, String)> = Vec::new();
        for nome_antigo in origem.sheet.nomes_das_tabelas() {
            let novo = crate::operacoes::tabela::nome_disponivel(&usados, "Tabela");
            usados.push(novo.clone());
            renomes.push((nome_antigo, novo));
        }
        let sheet = origem.sheet.duplicar_renomeando(nome, &renomes);
        let id = self.proximo_id;
        self.proximo_id += 1;
        let em = indice + 1;
        self.abas.insert(em, AbaViva { sheet, versao: 1, id });
        self.abas[em].sheet.manual.set(self.manual);
        // A cópia pode ler de fora (outras abas, nomes): revalidar.
        self.depois_de_mudar(usize::MAX);
        let assinatura = self.assinatura_das_abas();
        self.hist_abas.refazer.clear();
        self.hist_abas.desfazer.push(OpDuplicar { id, em, aba: None, assinatura });
        Some(em)
    }

    fn assinatura_das_abas(&self) -> Vec<(u32, u32)> {
        self.abas.iter().map(|a| (a.id, a.versao)).collect()
    }

    /// A última operação de abas ainda é a edição mais recente da pasta?
    pub fn pode_desfazer_aba(&self) -> bool {
        self.hist_abas.desfazer.last()
            .is_some_and(|op| op.assinatura == self.assinatura_das_abas())
    }

    pub fn pode_refazer_aba(&self) -> bool {
        self.hist_abas.refazer.last()
            .is_some_and(|op| op.assinatura == self.assinatura_das_abas())
    }

    /// Desfaz a última duplicação, se nada foi editado depois dela: tira a
    /// cópia da pasta. Devolve o índice que deve ficar ativo.
    pub fn desfazer_aba(&mut self) -> Option<usize> {
        if !self.pode_desfazer_aba() || self.abas.len() <= 1 {
            return None;
        }
        let mut op = self.hist_abas.desfazer.pop()?;
        let pos = self.indice_do_id(op.id)?;
        op.aba = Some(self.abas.remove(pos));
        self.depois_de_mudar(usize::MAX);
        op.assinatura = self.assinatura_das_abas();
        self.hist_abas.refazer.push(op);
        Some(pos.saturating_sub(1).min(self.abas.len() - 1))
    }

    /// Refaz a duplicação desfeita, recolocando a mesma cópia (mesmo `id`).
    /// Devolve o índice da cópia.
    pub fn refazer_aba(&mut self) -> Option<usize> {
        if !self.pode_refazer_aba() {
            return None;
        }
        let mut op = self.hist_abas.refazer.pop()?;
        let aba = op.aba.take()?;
        let em = op.em.min(self.abas.len());
        self.abas.insert(em, aba);
        self.abas[em].sheet.manual.set(self.manual);
        self.depois_de_mudar(usize::MAX);
        op.assinatura = self.assinatura_das_abas();
        self.hist_abas.desfazer.push(op);
        Some(em)
    }

    /// Renomeia a aba e reescreve as fórmulas que a citam, em todas as abas —
    /// o `renameSheet` do TypeScript. Recusa nome vazio, com `!` ou repetido.
    pub fn renomear_aba(&mut self, indice: usize, nome: &str) -> bool {
        let limpo = nome.trim();
        if indice >= self.abas.len() || limpo.is_empty() || limpo.contains('!') {
            return false;
        }
        let repetido = self
            .abas
            .iter()
            .enumerate()
            .any(|(i, a)| i != indice && a.sheet.name.to_lowercase() == limpo.to_lowercase());
        if repetido {
            return false;
        }
        self.hist_abas = HistoricoAbas::default();
        let antigo = std::mem::replace(&mut self.abas[indice].sheet.name, limpo.to_string());
        for i in 0..self.abas.len() {
            let mut edicoes = Vec::new();
            self.abas[i].sheet.for_each_snapshot(&mut |row, col, snap| {
                if snap.raw.starts_with('=') {
                    let novo = reescrever_aba(&snap.raw, &antigo, limpo);
                    if novo != snap.raw {
                        edicoes.push((row, col, novo));
                    }
                }
            });
            if edicoes.is_empty() {
                continue;
            }
            let aba = &mut self.abas[i];
            aba.sheet.abrir_transacao();
            for (row, col, raw) in edicoes {
                aba.sheet.set_cell(row, col, &raw);
            }
            aba.sheet.fechar_transacao();
            aba.versao = proxima_versao(aba.versao);
        }
        // Os intervalos nomeados guardam a aba pelo texto, não pelo id.
        // Sem esta migração, um nome deixa de funcionar após renomear a aba.
        for entrada in self.nomeados.values_mut() {
            if entrada.area.sheet.as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case(&antigo))
            {
                entrada.area.sheet = Some(limpo.to_owned());
            }
        }
        self.depois_de_mudar(usize::MAX);
        true
    }

    // --------------------------------------------------- intervalos nomeados

    /// Define (ou redefine) um nome. A validação da grafia é de quem chama:
    /// `isValidName` no frontend TypeScript ou `nomes_ui::validar_nome` na GUI Iced.
    pub fn definir_nome(&mut self, nome: &str, area: Rect) {
        let limpo = nome.trim().to_string();
        self.nomeados.insert(limpo.to_uppercase(), Nomeado { nome: limpo, area });
        self.depois_de_mudar(usize::MAX);
    }

    pub fn remover_nome(&mut self, nome: &str) -> bool {
        let saiu = self.nomeados.remove(&nome.trim().to_uppercase()).is_some();
        if saiu {
            self.depois_de_mudar(usize::MAX);
        }
        saiu
    }

    /// Os nomes, na ordem alfabética.
    pub fn nomeados(&self) -> Vec<Nomeado> {
        let mut v: Vec<Nomeado> = self.nomeados.values().cloned().collect();
        v.sort_by_key(|a| a.nome.to_lowercase());
        v
    }

    // ------------------------------------------------------------- valores

    /// O valor de uma célula com a pasta em volta: `Dados!A1` e os nomes
    /// funcionam. É por aqui que o quadro e os comandos devem ler.
    pub fn valor(&self, indice: usize, row: u32, col: u32) -> Value {
        match self.abas.get(indice) {
            Some(a) => a.sheet.get_value_em(row, col, Some(self)),
            None => Value::Empty,
        }
    }

    /// Uma árvore avulsa avaliada na célula, com a pasta em volta
    /// ([`Sheet::avaliar_em`]).
    pub fn avaliar_em(&self, indice: usize, row: u32, col: u32, node: &crate::formula::ast::Node) -> Value {
        match self.abas.get(indice) {
            Some(a) => a.sheet.avaliar_em(row, col, node, Some(self)),
            None => Value::Empty,
        }
    }

    /// Roda uma mudança qualquer na aba e cuida da versão e das fórmulas que
    /// leem de fora. Para os comandos que ainda não têm método próprio.
    pub fn editar<F>(&mut self, indice: usize, acao: F) -> Option<Mudanca>
    where
        F: FnOnce(&mut Sheet),
    {
        let sujo = self.abas.get(indice)?.sheet.used_rect();
        // As células que a ação tocou dizem quais fórmulas de fora sujar:
        // apagar uma célula da Resumo não recalcula as SOMASES dela sobre a
        // Dados (3,4 s na Planilha Pesada, 18/10/2026). Sem saber, todas.
        let antes = std::mem::replace(&mut *self.abas.get(indice)?.sheet.rastro.borrow_mut(),
            crate::planilha::sheet::Rastro::sabido());
        // A ação pode escrever fora do retângulo usado de antes.
        self.mudar_com_alcance_por(indice, vec![sujo], |s| {
            acao(s);
            let mut rastro = s.rastro.borrow_mut();
            let tocadas = rastro.tirar();
            *rastro = antes;
            rastro.juntar(tocadas.as_deref());
            tocadas.as_deref().map(alcance_das_chaves)
        })
    }

    /// Edita um fragmento de uma transação grande sem disparar, a cada
    /// fragmento, uma nova versão de aba, recálculo de fórmulas externas
    /// e invalidação de caches gráficos. O chamador DEVE encerrar a
    /// operação com `editar` (commit) ou `editar` + `reverter_transacao`
    /// (cancelamento), que publicam a única mudança efetiva.
    ///
    /// Usado por "Converter texto em colunas" em 1M+ linhas; não é
    /// apropriado para edições avulsas que precisam atualizar a interface.
    pub fn editar_lote_sem_notificar<F>(&mut self, indice: usize, acao: F) -> bool
    where F: FnOnce(&mut Sheet) {
        let Some(aba) = self.abas.get_mut(indice) else { return false };
        acao(&mut aba.sheet);
        aba.sheet.tocar();
        true
    }

    /// Depois de mexer na aba `mexida`: toda fórmula que lê de fora (outra
    /// aba, um nome) pode ter mudado — suja e sobe a versão de quem tem. É o
    /// `invalidateExternal` do TypeScript: são poucas, e o cálculo é
    /// preguiçoso.
    fn depois_de_mudar(&mut self, mexida: usize) {
        self.depois_de_mudar_em(mexida, None);
    }

    /// [`Self::depois_de_mudar`] sabendo onde: com `alcance`, só as fórmulas
    /// que leem a aba `mexida` em alguma coluna desses retângulos são sujas
    /// (editar a Dados não recalcula um milhão de `PROCX` que leem Produtos).
    /// Sem `alcance` (não se sabe o que mudou), todas as que leem de fora.
    pub(crate) fn depois_de_mudar_em(&mut self, mexida: usize, alcance: Option<&[Rect]>) {
        if self.manual {
            self.externas_adiadas.push((mexida, alcance.map(<[Rect]>::to_vec)));
            return;
        }
        let nome = self.abas.get(mexida).map(|a| a.sheet.name.clone());
        for (i, aba) in self.abas.iter_mut().enumerate() {
            if !aba.sheet.tem_formulas_externas() {
                continue;
            }
            let sujou = match (&nome, alcance) {
                (Some(nome), Some(rects)) => aba.sheet.sujar_externas_de(nome, rects),
                _ => {
                    aba.sheet.sujar_externas();
                    true
                }
            };
            if sujou && i != mexida {
                aba.versao = proxima_versao(aba.versao);
            }
        }
    }

    /// Calcula em paralelo (todas as threads escolhidas em
    /// [`crate::paralelismo`]) as fórmulas empacotadas sujas da pasta, coluna
    /// por coluna na ordem das dependências. O que não cabe nesse desenho
    /// continua sujo para o cálculo preguiçoso. Devolve quantas calculou.
    /// Ver [`crate::planilha::calculo_paralelo`].
    pub fn calcular_em_paralelo(&mut self) -> usize {
        // As matrizes derramam antes: as threads leem as colunas direto e
        // precisam saber onde há área derramada (fica fora do paralelo).
        for aba in &self.abas {
            aba.sheet.garantir_derramamentos(Some(self as &dyn Hospedeiro));
        }
        crate::planilha::calculo_paralelo::calcular(&mut self.abas, &self.funcoes_externas)
    }

    /// [`Sheet::consolidar_calculos`] nas abas, até `limite` no total.
    pub fn consolidar_calculos(&mut self, limite: usize) -> usize {
        let mut movidos = 0;
        for aba in &mut self.abas {
            movidos += aba.sheet.consolidar_calculos(limite - movidos);
            if movidos >= limite {
                break;
            }
        }
        movidos
    }

    /// Ver [`Sheet::soltar_faixas`], em todas as abas.
    pub fn soltar_faixas(&self) {
        for aba in &self.abas {
            aba.sheet.soltar_faixas();
        }
    }

    /// Ver [`Sheet::calculo_pendente`].
    pub fn calculo_pendente(&self, indice: usize, row: u32, col: u32) -> bool {
        self.abas.get(indice).is_some_and(|a| a.sheet.calculo_pendente(row, col))
    }

    pub fn aba(&self, indice: usize) -> Option<&AbaViva> {
        self.abas.get(indice)
    }

    // ------------------------------------------------ pastas externas

    /// Os arquivos que alguma fórmula citou e ainda não estão carregados.
    /// Quem lê o disco é a ponte, que devolve com [`Self::instalar_externo`].
    pub fn externos_pedidos(&self) -> Vec<String> {
        self.externos.tirar_pedidos()
    }

    /// Os arquivos externos já carregados, como foram escritos.
    pub fn externos_carregados(&self) -> Vec<String> {
        self.externos.carregados()
    }

    /// Troca valores avulsos de uma pasta externa (a outra janela mandou) e
    /// recalcula só quem lê essas células. `false` quando nada mudou ou ela
    /// não está carregada. Ver [`Self::atualizar_externo_em`].
    pub fn atualizar_externo(&mut self, arquivo: &str, aba: &str, celulas: &[(u32, u32, Value)]) -> bool {
        self.atualizar_externo_em(arquivo, aba, celulas, None)
    }

    /// Põe (ou troca) os valores de uma pasta externa e recalcula só quem lê
    /// aquele arquivo (as fórmulas que leem de fora estão nas `externas` de
    /// cada aba). O caminho e a data guardados ficam como estavam.
    pub fn instalar_externo(&mut self, arquivo: &str, abas: Vec<Sheet>) -> bool {
        let mudou = self.externos.instalar(arquivo, abas);
        if mudou {
            self.sujar_quem_le_vinculo(arquivo, None, None);
        }
        mudou
    }

    pub fn aba_mut(&mut self, indice: usize) -> Option<&mut AbaViva> {
        // Quem pede a aba para escrever pode mudar qualquer valor dela.
        let aba = self.abas.get_mut(indice)?;
        aba.sheet.tocar();
        Some(aba)
    }

    /// O índice da aba com este nome, sem diferenciar maiúscula de minúscula
    /// — é como o resto do motor trata nome de aba.
    pub fn indice_por_nome(&self, nome: &str) -> Option<usize> {
        self.abas
            .iter()
            .position(|a| a.sheet.name.to_lowercase() == nome.to_lowercase())
    }

    /// A versão da aba, ou zero quando ela não existe.
    pub fn versao(&self, indice: usize) -> u32 {
        self.abas.get(indice).map(|a| a.versao).unwrap_or(0)
    }

    /// Roda uma mudança numa aba e cuida da versão.
    ///
    /// Todo caminho que mexe na planilha passa por aqui — é o único lugar que
    /// sobe a versão, e ter um só é o que garante que nenhum comando novo
    /// esqueça de fazê-lo e deixe a tela mostrando conteúdo velho.
    fn mudar<F>(&mut self, indice: usize, sujos: Vec<Rect>, acao: F) -> Option<Mudanca>
    where
        F: FnOnce(&mut Sheet),
    {
        self.mudar_com_alcance(indice, sujos, None, acao)
    }

    /// `alcance`: os retângulos que a ação escreve, quando se sabe com
    /// certeza — só as fórmulas de outras abas que leem ali são sujas. `None`
    /// (o padrão de [`Self::mudar`]): todas as que leem de fora. Os `sujos`
    /// são o que a tela redesenha e podem ser maiores.
    fn mudar_com_alcance<F>(&mut self, indice: usize, sujos: Vec<Rect>, alcance: Option<Vec<Rect>>, acao: F) -> Option<Mudanca>
    where
        F: FnOnce(&mut Sheet),
    {
        self.mudar_com_alcance_por(indice, sujos, |s| { acao(s); alcance })
    }

    /// [`Self::mudar_com_alcance`] com o alcance que a própria ação devolve.
    fn mudar_com_alcance_por<F>(&mut self, indice: usize, sujos: Vec<Rect>, acao: F) -> Option<Mudanca>
    where
        F: FnOnce(&mut Sheet) -> Option<Vec<Rect>>,
    {
        let aba = self.abas.get_mut(indice)?;
        let alcance = acao(&mut aba.sheet);
        aba.sheet.tocar();
        aba.versao = proxima_versao(aba.versao);
        let versao = aba.versao;
        self.depois_de_mudar_em(indice, alcance.as_deref());
        Some(Mudanca { versao, sujos })
    }

    /// Escreve o texto digitado numa célula.
    ///
    /// O retângulo sujo não é só a célula: qualquer fórmula da aba pode
    /// depender dela. Por enquanto a aba inteira é declarada suja, que é
    /// grosseiro e correto — afinar isto (perguntando ao grafo quem depende)
    /// é uma otimização, e otimizar antes de medir foi o erro que esta
    /// sessão inteira passou consertando.
    pub fn escrever_celula(
        &mut self,
        indice: usize,
        row: u32,
        col: u32,
        texto: String,
    ) -> Option<Mudanca> {
        let sujo = self
            .abas
            .get(indice)
            .map(|a| a.sheet.used_rect())
            .unwrap_or(Rect { r0: row, c0: col, r1: row, c1: col, sheet: None });
        let celula = Rect { r0: row, c0: col, r1: row, c1: col, sheet: None };
        self.mudar_com_alcance(indice, vec![sujo], Some(vec![celula]), |s| s.set_cell(row, col, &texto))
    }

    /// Exclui colunas inteiras.
    ///
    /// Barato: o armazenamento é colunar, então isto é troca de endereço, não
    /// cópia de célula. É a mesma conta que o `ops.ts` do TypeScript passou a
    /// fazer em 22/09/2026 — aqui ela já nasce assim.
    pub fn excluir_colunas(&mut self, indice: usize, at: u32, count: u32) -> Option<Mudanca> {
        self.mover_estrutura(indice, true, at, count, true)
    }

    pub fn inserir_colunas(&mut self, indice: usize, at: u32, count: u32) -> Option<Mudanca> {
        self.mover_estrutura(indice, true, at, count, false)
    }

    pub fn excluir_linhas(&mut self, indice: usize, at: u32, count: u32) -> Option<Mudanca> {
        self.mover_estrutura(indice, false, at, count, true)
    }

    pub fn inserir_linhas(&mut self, indice: usize, at: u32, count: u32) -> Option<Mudanca> {
        self.mover_estrutura(indice, false, at, count, false)
    }

    /// Insere ou exclui linhas (`coluna == false`) ou colunas, e conserta o
    /// texto das fórmulas — desta aba e das outras que apontam para ela. É o
    /// `moverColunas` do `ops.ts`, nos mesmos três passos e na mesma ordem:
    ///
    /// 1. **antes de mover**, anotar as fórmulas que precisam de conserto. A
    ///    fórmula avulsa é sempre anotada; a colunar é perguntada por
    ///    PADRÃO — uma coluna de dois milhões de fórmulas é uma pergunta;
    /// 2. deslocar (troca de chave, sem copiar dado);
    /// 3. reescrever o texto das anotadas, já na posição nova. A referência
    ///    que caiu no que foi excluído vira `#REF!`.
    ///
    /// O deslocamento não entra no desfazer (como no TypeScript); o
    /// conserto do texto entra.
    pub fn mover_estrutura(
        &mut self,
        indice: usize,
        coluna: bool,
        at: u32,
        count: u32,
        removing: bool,
    ) -> Option<Mudanca> {
        let sujo = self.abas.get(indice)?.sheet.used_rect();
        if count == 0 {
            return self.mudar_com_alcance(indice, vec![sujo], None, |_| {});
        }
        let nome = self.abas[indice].sheet.name.clone();
        let destino = |p: u32| -> Option<u32> {
            if !removing {
                return Some(if p >= at { p + count } else { p });
            }
            if p >= at && p < at + count {
                return None;
            }
            Some(if p >= at + count { p - count } else { p })
        };

        // 1) o que precisa de conserto, anotado antes de mover.
        let mut consertos: Vec<(usize, Vec<Conserto>)> = Vec::new();
        for (j, aba) in self.abas.iter().enumerate() {
            let s = &aba.sheet;
            let mesma = j == indice;
            let mut lista = Vec::new();
            for (&k, cell) in &s.cells {
                if cell.raw.starts_with('=') {
                    lista.push((crate::celulas::refs::key_row(k), crate::celulas::refs::key_col(k), cell.raw.clone()));
                }
            }
            for uso_col in s.formulas.columns_in_use() {
                let col = uso_col.col;
                for uso in &uso_col.usos {
                    // Intervalo local entra pelas pontas, e só na própria
                    // aba: em outra, `A2:A10` não é desta.
                    let refs: std::borrow::Cow<[crate::formula::formula_pattern::PatternRef]> = if mesma && !uso.faixas.is_empty() {
                        uso.refs.iter().cloned().chain(uso.faixas.iter().flat_map(|f| f.pontas_como_refs())).collect::<Vec<_>>().into()
                    } else {
                        uso.refs.as_slice().into()
                    };
                    // A pergunta é por padrão. Sem saber a linha (o padrão
                    // vale para a coluna toda), a linha entra no teste
                    // célula a célula só quando o eixo é de linha.
                    if coluna {
                        if !precisa_conserto(mesma, col, &refs, coluna, &destino) {
                            continue;
                        }
                        s.formulas.for_each_with_pattern(col, uso.id, |row| {
                            if let Some(raw) = s.formulas.raw_at(row, col) {
                                lista.push((row, col, raw));
                            }
                        });
                    } else {
                        s.formulas.for_each_with_pattern(col, uso.id, |row| {
                            if precisa_conserto(mesma, row, &refs, coluna, &destino) {
                                if let Some(raw) = s.formulas.raw_at(row, col) {
                                    lista.push((row, col, raw));
                                }
                            }
                        });
                    }
                }
            }
            if !lista.is_empty() {
                consertos.push((j, lista));
            }
        }

        // 2) o deslocamento.
        self.abas[indice].sheet.deslocar(coluna, at, count, removing);
        self.deslocar_nomeados(&nome, coluna, at, count, removing);

        // 3) o texto, na posição nova.
        let eixo = if coluna { crate::formula::formula_text::Axis::Col } else { crate::formula::formula_text::Axis::Row };
        for (j, lista) in consertos {
            let mesma = j == indice;
            let alvo = &mut self.abas[j];
            let proprio = alvo.sheet.name.clone();
            alvo.sheet.abrir_transacao();
            alvo.sheet.adiar_cascata();
            for (row, col, raw) in lista {
                let (novo_r, novo_c) = if !mesma {
                    (Some(row), Some(col))
                } else if coluna {
                    (Some(row), destino(col))
                } else {
                    (destino(row), Some(col))
                };
                let (Some(nr), Some(nc)) = (novo_r, novo_c) else {
                    continue;
                };
                let proximo = crate::formula::formula_text::adjust_for_structure(
                    &raw,
                    crate::formula::formula_text::StructureAdjustOptions {
                        axis: eixo,
                        at: at as i32,
                        count: count as i32,
                        removing,
                        sheet: &nome,
                        own_sheet: &proprio,
                    },
                );
                if proximo != raw || nr != row || nc != col {
                    alvo.sheet.set_cell(nr, nc, &proximo);
                }
            }
            alvo.sheet.soltar_cascata();
            alvo.sheet.fechar_transacao();
            if !mesma {
                alvo.versao = proxima_versao(alvo.versao);
            }
        }
        // Estrutura: tudo pode ter mudado de lugar.
        self.mudar_com_alcance(indice, vec![sujo], None, |_| {})
    }
}

impl Workbook {
    /// Os intervalos nomeados da aba `aba` acompanham a linha/coluna
    /// inserida ou excluída, como no Excel: inserir dentro estica, excluir
    /// dentro encolhe, e o nome cuja área foi toda excluída sai (não há
    /// nome "quebrado" para guardar o `#REF!`). shortcut: nome sem aba
    /// (`sheet: None`) não é tocado, porque não se sabe de qual aba ele é.
    fn deslocar_nomeados(&mut self, aba: &str, coluna: bool, at: u32, count: u32, removing: bool) {
        use crate::celulas::refs::{MAX_COLS, MAX_ROWS};
        let limite = if coluna { MAX_COLS } else { MAX_ROWS } - 1;
        let fim = at + count;
        self.nomeados.retain(|_, n| {
            if !n.area.sheet.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(aba)) {
                return true;
            }
            let (p0, p1) = if coluna { (n.area.c0, n.area.c1) } else { (n.area.r0, n.area.r1) };
            let (novo0, novo1) = if !removing {
                let anda = |p: u32| if p >= at { (p + count).min(limite) } else { p };
                (anda(p0), anda(p1))
            } else {
                if p0 >= at && p1 < fim {
                    return false;
                }
                let novo0 = if p0 < at { p0 } else if p0 < fim { at } else { p0 - count };
                let novo1 = if p1 < at { p1 } else if p1 < fim { at - 1 } else { p1 - count };
                (novo0, novo1)
            };
            if coluna {
                (n.area.c0, n.area.c1) = (novo0, novo1);
            } else {
                (n.area.r0, n.area.r1) = (novo0, novo1);
            }
            true
        });
    }

    /// Copia um bloco com fotos da origem anteriores à escrita, mesmo quando
    /// as áreas se sobrepõem. Fórmulas deslocam referências relativas; estilo
    /// direto e vazios também são copiados. Um Ctrl+Z desfaz a cópia inteira.
    pub fn copiar_intervalo(&mut self, indice: usize, origem: Rect, dr: i32, dc: i32) -> Option<Mudanca> {
        use crate::celulas::refs::{MAX_ROWS, MAX_COLS};
        use crate::operacoes::fill::shift_formula;
        use crate::planilha::sheet::edicao::Snap;

        let origem=origem.normalized();
        if dr==0 && dc==0 { return None; }
        let deslocar=|valor:u32,delta:i32,limite:u32| -> Option<u32> {
            let novo=valor as i64+delta as i64;
            (novo>=0 && novo<limite as i64).then_some(novo as u32)
        };
        let destino=Rect::new(
            deslocar(origem.r0,dr,MAX_ROWS)?,deslocar(origem.c0,dc,MAX_COLS)?,
            deslocar(origem.r1,dr,MAX_ROWS)?,deslocar(origem.c1,dc,MAX_COLS)?,
        );
        let aba=self.abas.get(indice)?;
        let fotos: Vec<(u32,u32,Option<Snap>)>=(origem.r0..=origem.r1)
            .flat_map(|r|(origem.c0..=origem.c1).map(move |c|(r,c)))
            .map(|(r,c)|(r,c,aba.sheet.snapshot(r,c))).collect();
        let sujo=aba.sheet.used_rect();
        self.mudar(indice,vec![sujo,origem,destino],|sheet|{
            sheet.abrir_transacao();
            for (r,c,foto) in fotos {
                let nova=foto.map(|mut snap|{
                    snap.raw=shift_formula(&snap.raw,dr,dc);
                    snap
                });
                sheet.apply(deslocar(r,dr,MAX_ROWS).unwrap(),
                    deslocar(c,dc,MAX_COLS).unwrap(),nova,true);
            }
            sheet.fechar_transacao();
        })
    }

    /// Move o bloco `origem` da aba `indice` `dr` linhas e `dc` colunas —
    /// o arrastar pela borda do Excel. Regras em [`crate::planilha::mover`]. Na aba
    /// mexida é um Ctrl+Z só; o conserto das fórmulas das outras abas entra
    /// no desfazer de cada uma, como no [`Self::mover_estrutura`]. `None`
    /// quando a aba não existe ou o destino sairia da planilha.
    pub fn mover_intervalo(&mut self, indice: usize, origem: Rect, dr: i32, dc: i32) -> Option<Mudanca> {
        use crate::planilha::mover::Movimento;
        use crate::planilha::sheet::edicao::Snap;

        let origem = Rect::new(origem.r0, origem.c0, origem.r1, origem.c1).normalized();
        let aba = self.abas.get(indice)?;
        let mov = Movimento { aba: aba.sheet.name.clone(), origem: origem.clone(), dr, dc };
        let destino = mov.destino()?;
        let mut sujo = aba.sheet.used_rect();
        if dr == 0 && dc == 0 {
            return None;
        }
        let dentro = |r: &Rect, row: u32, col: u32| row >= r.r0 && row <= r.r1 && col >= r.c0 && col <= r.c1;
        // Onde uma referência precisa cair para mudar: a origem ou o destino.
        let (lin0, lin1) = (origem.r0.min(destino.r0), origem.r1.max(destino.r1));
        let (col0, col1) = (origem.c0.min(destino.c0), origem.c1.max(destino.c1));

        // 1) o que vai: texto e estilo direto de cada célula da origem.
        let s = &aba.sheet;
        let movidas: Vec<(u32, u32, Snap)> = s
            .chaves_no_retangulo(&origem)
            .into_iter()
            .filter_map(|k| {
                let (row, col) = (crate::celulas::refs::key_row(k), crate::celulas::refs::key_col(k));
                s.snapshot(row, col).map(|snap| (row, col, snap))
            })
            .collect();
        let sobrescritas = s.chaves_no_retangulo(&destino);

        // 2) as fórmulas de fora do bloco que leem a origem ou o destino.
        let mut consertos: Vec<(usize, Vec<Conserto>)> = Vec::new();
        for (j, viva) in self.abas.iter().enumerate() {
            let s = &viva.sheet;
            let mesma = j == indice;
            let fora = |row: u32, col: u32| !mesma || (!dentro(&origem, row, col) && !dentro(&destino, row, col));
            let mut lista = Vec::new();
            let mut anotar = |row: u32, col: u32, raw: &str| {
                let novo = mov.reescrever(raw, &s.name);
                if novo != raw {
                    lista.push((row, col, novo));
                }
            };
            for (&k, cell) in &s.cells {
                let (row, col) = (crate::celulas::refs::key_row(k), crate::celulas::refs::key_col(k));
                if cell.raw.starts_with('=') && fora(row, col) {
                    anotar(row, col, &cell.raw);
                }
            }
            // A fórmula colunar é perguntada por padrão: só entra célula a
            // célula o padrão com uma referência que pode cair no bloco.
            for uso_col in s.formulas.columns_in_use() {
                let col = uso_col.col;
                for uso in &uso_col.usos {
                    let da_aba = |r: &crate::formula::formula_pattern::PatternRef| {
                        mov.aba.eq_ignore_ascii_case(r.sheet.as_deref().unwrap_or(&s.name))
                    };
                    let coluna_alvo = |r: &crate::formula::formula_pattern::PatternRef| {
                        if r.col_lock { r.col } else { col as i32 + r.col }
                    };
                    // Intervalo local: só na aba do bloco movido.
                    let faixas_daqui = mov.aba.eq_ignore_ascii_case(&s.name);
                    let pode = uso.refs.iter().any(|r| {
                        let c = coluna_alvo(r);
                        da_aba(r) && c >= col0 as i32 && c <= col1 as i32
                    }) || (faixas_daqui && uso.faixas.iter().any(|f| f.colunas_em(col)
                        .is_some_and(|(c0, c1)| c0 <= col1 && col0 <= c1)));
                    if !pode {
                        continue;
                    }
                    s.formulas.for_each_with_pattern(col, uso.id, |row| {
                        let cai = uso.refs.iter().any(|r| {
                            let l = if r.row_lock { r.row } else { row as i32 + r.row };
                            let c = coluna_alvo(r);
                            da_aba(r) && l >= lin0 as i32 && l <= lin1 as i32 && c >= col0 as i32 && c <= col1 as i32
                        }) || (faixas_daqui && uso.faixas.iter().any(|f| f.rect_em(row, col)
                            .is_some_and(|(r0, c0, r1, c1)| r0 <= lin1 && lin0 <= r1 && c0 <= col1 && col0 <= c1)));
                        if cai && fora(row, col) {
                            if let Some(raw) = s.formulas.raw_at(row, col) {
                                anotar(row, col, &raw);
                            }
                        }
                    });
                }
            }
            if !lista.is_empty() {
                consertos.push((j, lista));
            }
        }

        // 3) escreve: esvazia origem e destino, põe o bloco no lugar novo
        //    (com as referências internas acompanhando) e conserta as outras.
        let nome = mov.aba.clone();
        let alvo = &mut self.abas[indice].sheet;
        alvo.abrir_transacao();
        for &k in &sobrescritas {
            alvo.apply(crate::celulas::refs::key_row(k), crate::celulas::refs::key_col(k), None, true);
        }
        for (row, col, _) in &movidas {
            alvo.apply(*row, *col, None, true);
        }
        for (row, col, snap) in movidas {
            let raw = if snap.raw.starts_with('=') { mov.reescrever(&snap.raw, &nome) } else { snap.raw };
            let (nr, nc) = ((row as i64 + dr as i64) as u32, (col as i64 + dc as i64) as u32);
            alvo.apply(nr, nc, Some(Snap { raw, style: snap.style }), true);
        }
        for (j, lista) in consertos {
            let s = &mut self.abas[j].sheet;
            if j != indice {
                s.abrir_transacao();
            }
            for (row, col, novo) in lista {
                let style = s.snapshot(row, col).and_then(|a| a.style);
                s.apply(row, col, Some(Snap { raw: novo, style }), true);
            }
            if j != indice {
                s.fechar_transacao();
                self.abas[j].versao = proxima_versao(self.abas[j].versao);
            }
        }
        self.abas[indice].sheet.fechar_transacao();

        for r in [&origem, &destino] {
            sujo = Rect::new(sujo.r0.min(r.r0), sujo.c0.min(r.c0), sujo.r1.max(r.r1), sujo.c1.max(r.c1));
        }
        self.mudar(indice, vec![sujo], |_| {})
    }
}

/// Uma fórmula anotada para conserto: linha, coluna e o texto de antes.
type Conserto = (u32, u32, String);

/// Se uma fórmula colunar precisa ter o texto reescrito depois do corte —
/// o `precisaConserto` do `ops.ts`. `pos` é a coluna (ou a linha) da
/// fórmula. O padrão é relativo, e é isso que torna o caso comum de graça:
/// só pede conserto a referência que cai no que foi excluído, a travada com
/// `$`, e a que tem o corte passando entre ela e a fórmula.
fn precisa_conserto(
    mesma_aba: bool,
    pos: u32,
    refs: &[crate::formula::formula_pattern::PatternRef],
    coluna: bool,
    destino: &dyn Fn(u32) -> Option<u32>,
) -> bool {
    // Referências qualificadas (inclusive pontas de faixas) precisam da
    // reescrita que conhece o nome da aba alterada. A antiga otimização só
    // tratava referências locais: antes, todas as externas eram avulsas.
    if refs.iter().any(|r| r.sheet.is_some()) {
        return true;
    }
    if !mesma_aba {
        return refs.iter().any(|r| r.sheet.is_none());
    }
    let Some(novo_da_formula) = destino(pos) else {
        return false;
    };
    for r in refs {
        if r.sheet.is_some() {
            continue;
        }
        let (desloc, travada) = if coluna { (r.col, r.col_lock) } else { (r.row, r.row_lock) };
        let alvo = if travada { desloc } else { pos as i32 + desloc };
        if alvo < 0 {
            return true;
        }
        let Some(novo_alvo) = destino(alvo as u32) else {
            return true;
        };
        let depois = if travada { novo_alvo as i32 } else { novo_alvo as i32 - novo_da_formula as i32 };
        if desloc != depois {
            return true;
        }
    }
    false
}

/// A versão zero significa "nunca vi": se a volta ao zero acontecer depois
/// de quatro bilhões de edições, pula para 1 em vez de mentir que o quadro do
/// TypeScript ainda vale.
pub(crate) fn proxima_versao(v: u32) -> u32 {
    match v.wrapping_add(1) {
        0 => 1,
        n => n,
    }
}

impl Hospedeiro for Workbook {
    fn aba(&self, nome: &str) -> Option<&Sheet> {
        if let Some(a) = self.abas.iter().find(|a| crate::planilha::sheet::mesmo_nome(&a.sheet.name, nome)) {
            return Some(&a.sheet);
        }
        // `[Vendas.fshz]Dados` ou `/pasta/[Vendas.fshz]Dados`: outro arquivo.
        if crate::planilha::vinculo::e_externa(nome) {
            return self.externos.aba(nome);
        }
        None
    }

    fn aba_no_endereco(&self, endereco: usize) -> Option<&Sheet> {
        self.abas.iter().map(|a| &a.sheet).find(|s| *s as *const Sheet as usize == endereco)
    }

    fn nome(&self, nome: &str) -> Option<Rect> {
        if let Some(r) = self.resolver_estruturada(nome) {
            return Some(r);
        }
        self.nomeados.get(&nome.to_uppercase()).map(|n| n.area.clone())
    }

    fn funcao_externa(&self, nome: &str) -> bool {
        self.funcoes_externas.contains(&nome.to_uppercase())
    }

    fn nomes_das_abas(&self) -> Vec<String> {
        self.abas.iter().map(|a| a.sheet.name.clone()).collect()
    }
}

struct TabelaMeta {
    nome: String,
    display_name: String,
    r0: u32,
    c0: u32,
    r1: u32,
    c1: u32,
    tem_cabecalho: bool,
    #[allow(dead_code)] // a linha de totais mora fora de `area`; mantido do .fshz
    mostrar_totais: bool,
}

fn extrair_tabelas_de_tb(tb_str: &str) -> Vec<TabelaMeta> {
    let mut out = Vec::new();
    for chunk in tb_str.split('{') {
        if !chunk.contains("area") && !chunk.contains("nome") {
            continue;
        }
        let get_str = |key: &str| -> String {
            if let Some(pos) = chunk.find(&format!("\"{}\"", key)) {
                let rest = &chunk[pos + key.len() + 2..];
                if let Some(colon) = rest.find(':') {
                    let after_colon = rest[colon + 1..].trim();
                    if after_colon.starts_with('"') {
                        let inner = &after_colon[1..];
                        if let Some(quote_end) = inner.find('"') {
                            return inner[..quote_end].to_string();
                        }
                    }
                }
            }
            String::new()
        };

        let get_u32 = |key: &str| -> Option<u32> {
            if let Some(pos) = chunk.find(&format!("\"{}\"", key)) {
                let rest = &chunk[pos + key.len() + 2..];
                if let Some(colon) = rest.find(':') {
                    let after_colon = rest[colon + 1..].trim();
                    let num_str: String = after_colon.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(v) = num_str.parse::<u32>() {
                        return Some(v);
                    }
                }
            }
            None
        };

        if let (Some(r0), Some(c0), Some(r1), Some(c1)) = (get_u32("r0"), get_u32("c0"), get_u32("r1"), get_u32("c1")) {
            let nome = get_str("nome");
            let display_name = get_str("displayName");
            let tem_cabecalho = !chunk.contains("\"temCabecalho\":false");
            let mostrar_totais = chunk.contains("\"mostrarTotais\":true") || chunk.contains("\"hasTotals\":true");
            out.push(TabelaMeta {
                nome,
                display_name,
                r0,
                c0,
                r1,
                c1,
                tem_cabecalho,
                mostrar_totais,
            });
        }
    }
    out
}

impl Workbook {
    /// Tenta resolver uma referência estruturada do Excel (ex: `PROJETO_OLUBAJÉ[VALOR TOTAL (R$)]` ou `Tabela1[Valor]`).
    pub fn resolver_estruturada(&self, name: &str) -> Option<Rect> {
        let abre = name.find('[')?;
        if !name.ends_with(']') {
            return None;
        }
        let mut prefix = name[..abre].trim();
        let campo = name[abre + 1..name.len() - 1].trim();

        let mut target_sheet_name: Option<String> = None;
        if let Some(bang) = prefix.rfind('!') {
            let s_name = prefix[..bang].trim_matches('\'').trim();
            target_sheet_name = Some(s_name.to_string());
            prefix = prefix[bang + 1..].trim();
        }
        let raw_tabela = prefix.trim_matches('\'').trim();

        fn norm(s: &str) -> String {
            s.chars()
                .filter_map(|c| match c {
                    'Á' | 'À' | 'Â' | 'Ã' | 'á' | 'à' | 'â' | 'ã' => Some('a'),
                    'É' | 'Ê' | 'é' | 'ê' => Some('e'),
                    'Í' | 'í' => Some('i'),
                    'Ó' | 'Ô' | 'Õ' | 'ó' | 'ô' | 'õ' => Some('o'),
                    'Ú' | 'Ü' | 'ú' | 'ü' => Some('u'),
                    'Ç' | 'ç' => Some('c'),
                    '\u{0300}'..='\u{036f}' => None,
                    '_' => Some(' '),
                    other => Some(other.to_ascii_lowercase()),
                })
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        }

        let target_norm = norm(raw_tabela);
        let campo_norm = norm(campo);

        for aba_viva in &self.abas {
            let sheet = &aba_viva.sheet;
            if let Some(ref ts) = target_sheet_name {
                if norm(&sheet.name) != norm(ts) {
                    continue;
                }
            }

            // 1. Tentar resolver usando as tabelas registradas em sheet.tb (se existirem)
            if let Some(ref tb_json_str) = sheet.tb {
                let tabelas_meta = extrair_tabelas_de_tb(tb_json_str);
                let mut tabela_achada: Option<&TabelaMeta> = None;

                if !target_norm.is_empty() {
                    for t in &tabelas_meta {
                        let n1 = norm(&t.nome);
                        let n2 = norm(&t.display_name);
                        if n1 == target_norm || n2 == target_norm || n1.replace(' ', "") == target_norm.replace(' ', "") {
                            tabela_achada = Some(t);
                            break;
                        }
                        // Verificar células acima da tabela r0 - 1 e r0 - 2
                        if t.r0 > 0 {
                            for c_chk in t.c0..=t.c1 {
                                let cell_val1 = sheet.get_raw(t.r0 - 1, c_chk);
                                if !cell_val1.is_empty() && (norm(&cell_val1) == target_norm || norm(&cell_val1).replace(' ', "") == target_norm.replace(' ', "")) {
                                    tabela_achada = Some(t);
                                    break;
                                }
                                if t.r0 > 1 {
                                    let cell_val2 = sheet.get_raw(t.r0 - 2, c_chk);
                                    if !cell_val2.is_empty() && (norm(&cell_val2) == target_norm || norm(&cell_val2).replace(' ', "") == target_norm.replace(' ', "")) {
                                        tabela_achada = Some(t);
                                        break;
                                    }
                                }
                            }
                        }
                        if tabela_achada.is_some() {
                            break;
                        }
                    }
                }

                if tabela_achada.is_none() && target_norm.is_empty() && tabelas_meta.len() == 1 {
                    tabela_achada = Some(&tabelas_meta[0]);
                }

                if let Some(t) = tabela_achada {
                    let r0 = t.r0;
                    let c0 = t.c0;
                    let r1 = t.r1;
                    let c1 = t.c1;
                    let r_dados_start = if t.tem_cabecalho { r0 + 1 } else { r0 };
                    // A linha de totais mora ABAIXO de `area` (r1 é o último dado), no TS e no .fshz.
                    let mut r_dados_end = r1;

                    if r_dados_end >= r_dados_start {
                        let mut e_linha_de_totais = false;
                        for c_check in c0..=c1 {
                            let text_check = sheet.get_raw(r_dados_end, c_check);
                            let norm_tc = norm(&text_check);
                            if norm_tc == "total" || norm_tc == "total geral" || norm_tc.starts_with("total ") {
                                e_linha_de_totais = true;
                                break;
                            }
                            let raw_tc = text_check.trim();
                            if raw_tc.starts_with("=SOMA") || raw_tc.starts_with("=SUM") || raw_tc.starts_with("=SUBTOTAL") || raw_tc.starts_with("=SOMA.") {
                                e_linha_de_totais = true;
                                break;
                            }
                        }
                        if e_linha_de_totais && r_dados_end > r_dados_start {
                            r_dados_end -= 1;
                        }
                    }

                    if campo_norm == "#tudo" {
                        return Some(Rect { r0, c0, r1, c1, sheet: Some(sheet.name.clone()) });
                    }
                    if campo_norm == "#dados" {
                        return Some(Rect { r0: r_dados_start, c0, r1: r_dados_end, c1, sheet: Some(sheet.name.clone()) });
                    }
                    if campo_norm == "#cabecalhos" {
                        return Some(Rect { r0, c0, r1: r0, c1, sheet: Some(sheet.name.clone()) });
                    }

                    for c in c0..=c1 {
                        let cab = sheet.get_raw(r0, c);
                        let cab_n = norm(&cab);
                        if cab_n == campo_norm || cab_n.replace(' ', "") == campo_norm.replace(' ', "") {
                            return Some(Rect { r0: r_dados_start, c0: c, r1: r_dados_end, c1: c, sheet: Some(sheet.name.clone()) });
                        }
                    }
                }
            }

            // 2. Se não encontrou em sheet.tb, buscar no grid por célula de cabeçalho / título pertencente a target_norm
            let max_r = sheet.max_row.min(50);
            let max_c = sheet.max_col.min(200);

            let mut col_achada: Option<(u32, u32)> = None;

            if !target_norm.is_empty() {
                'busca_titulo: for r in 0..=max_r {
                    for c in 0..=max_c {
                        let val = sheet.get_raw(r, c);
                        if !val.is_empty() && (norm(&val) == target_norm || norm(&val).replace(' ', "") == target_norm.replace(' ', "")) {
                            for r_hdr in r..=std::cmp::min(r + 2, max_r) {
                                for c_hdr in c..=std::cmp::min(c + 20, max_c) {
                                    let v_hdr = sheet.get_raw(r_hdr, c_hdr);
                                    if !v_hdr.is_empty() && norm(&v_hdr) == campo_norm {
                                        col_achada = Some((r_hdr, c_hdr));
                                        break 'busca_titulo;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if col_achada.is_none() {
                'procura: for r in 0..=max_r {
                    for c in 0..=max_c {
                        let val = sheet.get_raw(r, c);
                        if !val.is_empty() && norm(&val) == campo_norm {
                            col_achada = Some((r, c));
                            break 'procura;
                        }
                    }
                }
            }

            if let Some((r_cabecalho, col)) = col_achada {
                let r0 = r_cabecalho + 1;
                let mut r1 = r0;
                let last_row = sheet.max_row;
                while r1 <= last_row {
                    let v = sheet.get_raw(r1, col);
                    let mut e_linha_de_totais = false;
                    let v_trim = v.trim();
                    if v_trim.starts_with("=SOMA") || v_trim.starts_with("=SUM") || v_trim.starts_with("=SUBTOTAL") || v_trim.starts_with("=SOMA.") {
                        e_linha_de_totais = true;
                    } else {
                        for c_check in 0..=max_c {
                            let text_check = sheet.get_raw(r1, c_check);
                            let norm_tc = norm(&text_check);
                            if norm_tc == "total" || norm_tc == "total geral" || norm_tc.starts_with("total ") {
                                e_linha_de_totais = true;
                                break;
                            }
                        }
                    }

                    if e_linha_de_totais {
                        break;
                    }

                    if v.is_empty() {
                        if r1 + 1 <= last_row && sheet.get_raw(r1 + 1, col).is_empty() {
                            break;
                        }
                    }
                    r1 += 1;
                }
                if r1 > r0 {
                    r1 -= 1;
                }
                if r0 <= r1 {
                    return Some(Rect {
                        r0,
                        c0: col,
                        r1,
                        c1: col,
                        sheet: Some(sheet.name.clone()),
                    });
                }
            }
        }

        None
    }
}

/// Letra que pode fazer parte de um nome de aba sem aspas: é a classe
/// `[A-Za-zÀ-ÿ0-9_.]` do TypeScript.
fn letra_de_nome(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.' || ('\u{C0}'..='\u{FF}').contains(&c)
}

/// Nome de aba em fórmula: compatibilidade com os chamadores anteriores.
/// A regra é centralizada em `celulas::refs::quote_sheet`, para não haver
/// duas implementações divergentes entre renomeação e parser de fórmulas.
pub fn nome_citado(nome: &str) -> String {
    crate::celulas::refs::quote_sheet(nome)
}

/// Troca `de!` por `para!` numa fórmula, com e sem aspas, sem ligar para
/// maiúscula — o `rewriteSheetReferences` do TypeScript, que fazia com
/// expressão regular: `(^|[^A-Za-zÀ-ÿ0-9_.])(?:de|'de')!`.
pub fn reescrever_aba(formula: &str, de: &str, para: &str) -> String {
    let texto: Vec<char> = formula.chars().collect();
    let puro: Vec<char> = de.to_lowercase().chars().collect();
    let citado: Vec<char> = format!("'{}'", de.replace('\'', "''")).to_lowercase().chars().collect();
    let destino = format!("{}!", nome_citado(para));
    let casa = |i: usize, alvo: &[char]| -> bool {
        if i + alvo.len() + 1 > texto.len() {
            return false;
        }
        let pedaco: String = texto[i..i + alvo.len()].iter().collect();
        let pedaco: Vec<char> = pedaco.to_lowercase().chars().collect();
        pedaco == alvo && texto[i + alvo.len()] == '!'
    };
    let mut saida = String::with_capacity(formula.len());
    let mut i = 0;
    while i < texto.len() {
        // Depois de `]` é a aba de outro arquivo (`[Vendas.fshz]Dados!`):
        // renomear a aba "Dados" daqui não mexe nela.
        let fronteira = i == 0 || !(letra_de_nome(texto[i - 1]) || texto[i - 1] == ']');
        if fronteira {
            if casa(i, &puro) {
                saida.push_str(&destino);
                i += puro.len() + 1;
                continue;
            }
            if casa(i, &citado) {
                saida.push_str(&destino);
                i += citado.len() + 1;
                continue;
            }
        }
        saida.push(texto[i]);
        i += 1;
    }
    saida
}

/// Os outros arquivos citados pelas fórmulas (`[Vendas.fshz]Dados!A1`).
pub mod vinculos;

#[cfg(test)]
mod tests;

/// As células tocadas como um retângulo por coluna (da primeira à última
/// linha tocada nela): quem lê de fora pergunta por coluna.
fn alcance_das_chaves(chaves: &[CellKey]) -> Vec<Rect> {
    let mut por_coluna: std::collections::BTreeMap<u32, (u32, u32)> = Default::default();
    for &k in chaves {
        let (r, c) = (key_row(k), key_col(k));
        let e = por_coluna.entry(c).or_insert((r, r));
        e.0 = e.0.min(r);
        e.1 = e.1.max(r);
    }
    por_coluna.into_iter().map(|(c, (r0, r1))| Rect { r0, c0: c, r1, c1: c, sheet: None }).collect()
}
