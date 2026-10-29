//! Tabela dinâmica grande sem travar a tela (18/10/2026). A pasta mora na
//! thread da tela e guarda caches que outra thread não pode tocar; por isso
//! a conta não lê a pasta direto. Em vez disso:
//!
//! 1. a tela lê só as colunas que a conta usa, em fatias de ~25 ms por
//!    pulso (a janela continua respondendo), para uma cópia compacta de
//!    8 bytes por célula ([`ColunaLida`]);
//! 2. a conta roda numa thread de fundo com essa cópia;
//! 3. a tela grava o resultado, pelo mesmo caminho de sempre.
//!
//! As colunas lidas ficam guardadas enquanto a origem não muda: mexer nos
//! campos depois só refaz a conta. A barra de baixo mostra o andamento.
use super::*;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use feathersheet_core::dinamica::dinamica_layout::{self, Resultado};

/// Acima disto (linhas da origem) a conta vai para a thread de fundo.
pub(super) const LINHAS_FUNDO: u32 = 200_000;
/// Quanto a tela lê por pulso.
const ORCAMENTO: Duration = Duration::from_millis(25);
/// Linhas da sonda que descobre quais colunas a conta lê.
const SONDA: usize = 64;
/// Marca dos não números (NaN negativo + índice no dicionário).
const MARCA: u64 = 0xFFF8_0000_0000_0000;

/// Uma coluna da origem copiada para a conta de fundo, linha 1 em diante.
/// Número fica como `f64`; texto, vazio, erro e lógico viram um NaN que
/// aponta para o dicionário (texto repetido custa um índice).
pub(in crate::app) struct ColunaLida {
    bits: Vec<u64>,
    dicionario: Vec<Value>,
    indices: HashMap<String, u32>,
}

impl ColunaLida {
    fn nova(linhas: usize) -> Self {
        Self { bits: Vec::with_capacity(linhas), dicionario: vec![Value::Empty], indices: HashMap::new() }
    }

    fn por(&mut self, v: Value) {
        let bits = match v {
            Value::Number(n) if !n.is_nan() => n.to_bits(),
            Value::Empty => MARCA,
            outro => {
                let chave = match &outro {
                    Value::Text(t) => format!("t{t}"),
                    o => format!("o{o:?}"),
                };
                let i = match self.indices.get(&chave) {
                    Some(&i) => i,
                    None => {
                        let i = self.dicionario.len() as u32;
                        self.indices.insert(chave, i);
                        self.dicionario.push(outro);
                        i
                    }
                };
                MARCA | u64::from(i)
            }
        };
        self.bits.push(bits);
    }

    /// O valor da linha `i` (a partir de 0 = linha 1 da origem).
    fn valor(&self, i: usize) -> Value {
        let Some(&b) = self.bits.get(i) else { return Value::Empty };
        let n = f64::from_bits(b);
        if !n.is_nan() {
            return Value::Number(n);
        }
        self.dicionario.get((b & 0xFFFF_FFFF) as usize).cloned().unwrap_or(Value::Empty)
    }

    /// Terminou a leitura: o índice de textos não serve mais à conta.
    fn fechar(mut self) -> Self {
        self.indices = HashMap::new();
        self.bits.shrink_to_fit();
        self
    }
}

/// O resultado da thread de fundo (o `Msg` precisa de `Clone`).
#[derive(Clone)]
pub struct Pronto(Arc<Mutex<Option<Resultado>>>);
impl std::fmt::Debug for Pronto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("Pronto") }
}

struct Trabalho {
    geracao: u64,
    destino: usize,
    estado: Estado,
    /// Colunas relativas à origem que a conta lê.
    colunas: Vec<usize>,
    total: usize,
    calculando: bool,
    /// Já mandou calcular as fórmulas da origem em fundo (uma vez só: o que o
    /// paralelo não alcança fica para a leitura).
    calculou_origem: bool,
}

/// O que fica guardado entre uma conta e outra.
#[derive(Default)]
pub(in crate::app) struct Fundo {
    /// (aba de origem, versão dela, área): mudou, as colunas lidas caem.
    chave: Option<(usize, u64, [u32; 4])>,
    cabecalhos: Arc<Vec<Value>>,
    prontas: HashMap<usize, Arc<ColunaLida>>,
    lendo: Option<(usize, ColunaLida)>,
    trabalho: Option<Trabalho>,
    geracao: u64,
}

impl Fundo {
    pub(in crate::app) fn ativo(&self) -> bool {
        self.trabalho.is_some()
    }

    /// Solta as colunas lidas (painel fechado, nada em andamento).
    pub(in crate::app) fn soltar(&mut self) {
        if self.trabalho.is_none() {
            *self = Fundo { geracao: self.geracao, ..Fundo::default() };
        }
    }

    /// (fração lida 0..1, calculando).
    fn andamento(&self) -> Option<(f32, bool)> {
        let t = self.trabalho.as_ref()?;
        if t.calculando {
            return Some((1.0, true));
        }
        let por_coluna = t.total.saturating_sub(1).max(1) as f32;
        let feitas = t.colunas.iter().filter(|c| self.prontas.contains_key(c)).count() as f32;
        let parcial = self.lendo.as_ref()
            .filter(|(c, _)| t.colunas.contains(c))
            .map_or(0.0, |(_, col)| col.bits.len() as f32 / por_coluna);
        Some((((feitas + parcial) / t.colunas.len().max(1) as f32).min(1.0), false))
    }
}

impl App {
    /// A conta desta dinâmica vai para o fundo? Origem grande e sem campo
    /// calculado (o campo calculado lê várias colunas por fórmula).
    pub(super) fn dinamica_vai_para_fundo(e: &Estado) -> bool {
        e.calculados.is_empty() && e.origem.r1.saturating_sub(e.origem.r0) > LINHAS_FUNDO
            && !(e.linhas.is_empty() && e.colunas.is_empty() && e.valores.is_empty())
    }

    /// Começa (ou recomeça, com os campos de agora) a conta de fundo.
    pub(super) fn iniciar_dinamica_fundo(&mut self, destino: usize, e: &Estado) -> Result<(), String> {
        avancado::conferir_nomes_calculados(e)?;
        let config = avancado::configuracao_avancada(e)?;
        let total = (e.origem.r1 - e.origem.r0 + 1) as usize;
        let largura = (e.origem.c1 - e.origem.c0 + 1) as usize;
        let versao = self.workbook.aba(e.folha_origem).map_or(0, |a| u64::from(a.versao));
        let chave = (e.folha_origem, versao, [e.origem.r0, e.origem.c0, e.origem.r1, e.origem.c1]);
        if self.dinamica_fundo.chave.as_ref() != Some(&chave) {
            let geracao = self.dinamica_fundo.geracao;
            self.dinamica_fundo = Fundo { chave: Some(chave), geracao, ..Fundo::default() };
            let (folha, r0, c0) = (e.folha_origem, e.origem.r0, e.origem.c0);
            self.dinamica_fundo.cabecalhos = Arc::new((0..largura)
                .map(|c| self.workbook.valor(folha, r0, c0 + c as u32)).collect());
        }
        // A sonda: a conta em poucas linhas, anotando as colunas que ela lê.
        // Junto, as que a configuração cita (uma linha descartada pelo
        // filtro não chega a ler as medidas).
        let lidas = std::cell::RefCell::new(HashSet::new());
        {
            let (folha, r0, c0) = (e.folha_origem, e.origem.r0, e.origem.c0);
            let leitor = |r: usize, c: usize| {
                if r > 0 { lidas.borrow_mut().insert(c); }
                self.workbook.valor(folha, r0 + r as u32, c0 + c as u32)
            };
            let _ = dinamica_layout::montar_com_extras(total.min(SONDA), leitor, &config,
                &avancado::opcoes(e), &avancado::extras_vigentes(e));
        }
        let c0 = e.origem.c0 as usize;
        let mut colunas: HashSet<usize> = lidas.into_inner();
        colunas.extend(e.linhas.iter().chain(e.colunas.iter()).chain(e.valores.iter().map(|v| &v.coluna))
            .chain(e.filtros.iter().map(|f| &f.coluna)).map(|c| c.saturating_sub(c0)));
        let mut colunas: Vec<usize> = colunas.into_iter().filter(|&c| c < largura).collect();
        colunas.sort_unstable();
        self.dinamica_fundo.geracao += 1;
        self.dinamica_fundo.trabalho = Some(Trabalho {
            geracao: self.dinamica_fundo.geracao,
            destino,
            estado: e.clone(),
            colunas,
            total,
            calculando: false, calculou_origem: false,
        });
        self.status = tr!("Calculando a tabela dinâmica em segundo plano…").into();
        Ok(())
    }

    /// Um pulso: lê mais um pedaço; com tudo lido, manda a conta ao fundo.
    pub(super) fn passo_dinamica_fundo(&mut self) -> Task<Msg> {
        let Some(t) = self.dinamica_fundo.trabalho.as_ref() else { return Task::none() };
        if t.calculando { return Task::none(); }
        let inicio = Instant::now();
        let (folha, r0, c0) = (t.estado.folha_origem, t.estado.origem.r0, t.estado.origem.c0);
        let linhas = t.total.saturating_sub(1);
        let faltam: Vec<usize> = t.colunas.iter().copied()
            .filter(|c| !self.dinamica_fundo.prontas.contains_key(c)).collect();
        // Coluna de fórmula ainda sem conta: lida aqui, célula a célula, calculava a
        // cadeia inteira pelo caminho lento (o BLOCO do 2026_03 passou de 9 GB, 28/10/2026).
        // Calcula antes em paralelo, fora da janela, e o pulso seguinte lê pronto.
        if !t.calculou_origem && self.dinamica_fundo.lendo.is_none()
            && self.workbook.aba(folha).is_some_and(|a| faltam.iter().any(|&c| a.sheet.formulas.tem_suja(c0 + c as u32)))
        {
            if let Some(t) = self.dinamica_fundo.trabalho.as_mut() { t.calculou_origem = true; }
            self.calcular_em_fundo();
            return Task::none();
        }
        for c in faltam {
            let (_, mut coluna) = match self.dinamica_fundo.lendo.take() {
                Some((lida, col)) if lida == c => (lida, col),
                _ => (c, ColunaLida::nova(linhas)),
            };
            while coluna.bits.len() < linhas {
                let fim = (coluna.bits.len() + 4096).min(linhas);
                for i in coluna.bits.len()..fim {
                    coluna.por(self.workbook.valor(folha, r0 + 1 + i as u32, c0 + c as u32));
                }
                if inicio.elapsed() >= ORCAMENTO && coluna.bits.len() < linhas {
                    self.dinamica_fundo.lendo = Some((c, coluna));
                    return Task::none();
                }
            }
            self.dinamica_fundo.prontas.insert(c, Arc::new(coluna.fechar()));
            if inicio.elapsed() >= ORCAMENTO { return Task::none(); }
        }
        // Tudo lido: a conta vai para a thread de fundo.
        let Some(t) = self.dinamica_fundo.trabalho.as_mut() else { return Task::none() };
        t.calculando = true;
        let geracao = t.geracao;
        let e = t.estado.clone();
        let total = t.total;
        let colunas: HashMap<usize, Arc<ColunaLida>> = t.colunas.iter()
            .filter_map(|c| self.dinamica_fundo.prontas.get(c).map(|col| (*c, Arc::clone(col)))).collect();
        let cabecalhos = Arc::clone(&self.dinamica_fundo.cabecalhos);
        let (config, opcoes, extras) = match avancado::configuracao_avancada(&e) {
            Ok(config) => (config, avancado::opcoes(&e), avancado::extras_vigentes(&e)),
            Err(erro) => {
                self.dinamica_fundo.trabalho = None;
                self.status = erro;
                return Task::none();
            }
        };
        Task::perform(async move {
            tokio::task::spawn_blocking(move || {
                let leitor = |r: usize, c: usize| -> Value {
                    if r == 0 { return cabecalhos.get(c).cloned().unwrap_or(Value::Empty); }
                    colunas.get(&c).map_or(Value::Empty, |col| col.valor(r - 1))
                };
                dinamica_layout::montar_com_extras(total, leitor, &config, &opcoes, &extras)
            }).await.ok()
        }, move |r| Msg::Dinamica(Evento::FundoPronto(geracao, Pronto(Arc::new(Mutex::new(r))))))
    }

    /// A conta de fundo voltou: grava, se ainda é a conta mais nova.
    pub(super) fn dinamica_fundo_pronta(&mut self, geracao: u64, pronto: Pronto) {
        let Some(t) = self.dinamica_fundo.trabalho.take_if(|t| t.geracao == geracao) else { return };
        let Some(resultado) = pronto.0.lock().ok().and_then(|mut r| r.take()) else {
            self.status = tr!("A conta da tabela dinâmica falhou.").into();
            return;
        };
        avancado::guardar_caminhos(self, &t.estado, &resultado);
        match self.aplicar_com_resultado(t.destino, &t.estado, Some(resultado)) {
            Ok(()) => {
                // Como no `atualizar_ao_vivo`: os grupos recolhíveis do painel.
                if let Ok((d, _, meta)) = edicao::localizar_selecionada(self) {
                    if let Ok(novo) = edicao::ler_estado(self, d, &meta) {
                        if let Some(atual) = self.dinamica.as_mut().filter(|a| a.editando == t.estado.editando) {
                            atual.grupos = novo.grupos;
                        }
                    }
                }
            }
            Err(erro) => self.status = erro,
        }
        if self.dinamica.is_none() {
            self.dinamica_fundo.soltar();
        }
    }

    /// A barra de baixo enquanto a dinâmica é lida ou calculada.
    pub(in crate::app) fn view_progresso_dinamica(&self) -> Element<'_, Msg> {
        let Some((fracao, calculando)) = self.dinamica_fundo.andamento() else {
            return Space::new(0, 0.0).into();
        };
        let texto = if calculando {
            tr!("Tabela dinâmica: calculando…").to_string()
        } else {
            trf!("Tabela dinâmica: lendo a origem · {:.0}%", fracao * 100.0)
        };
        row![
            pelicano::progress_bar(0.0..=100.0, fracao * 100.0)
                .width(Length::Fixed(150.0)).height(6.0)
                // shortcut: o trilho sai na cor do tema (texto a 12%), não em COR_BORDA; o pelicano não deixa trocar o fundo da barra, mudar quando o ProgressBar ganhar `track`.
                .color(COR_AZUL_ACENTO()),
            text(texto).size(11).color(COR_TEXTO_MUTED()),
        ].spacing(8).align(pelicano::Align::Center).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn coluna_lida_guarda_numero_texto_vazio_e_erro() {
        let mut c = ColunaLida::nova(4);
        let erro = Value::erro(feathersheet_core::celulas::values::ErrorCode::Div0);
        for v in [Value::Number(1.5), Value::Text("Sul".into()), Value::Empty, Value::Text("Sul".into()),
            erro.clone(), Value::Number(f64::NAN), Value::Bool(true), Value::Number(-0.0)] {
            c.por(v);
        }
        let c = c.fechar();
        assert_eq!(c.valor(0), Value::Number(1.5));
        assert_eq!(c.valor(1), Value::Text("Sul".into()));
        assert_eq!(c.valor(2), Value::Empty);
        assert_eq!(c.valor(3), Value::Text("Sul".into()));
        assert_eq!(c.dicionario.len(), 5, "vazio + Sul + erro + NaN + lógico: o texto repetido não duplica");
        assert_eq!(c.valor(4), erro);
        assert!(matches!(c.valor(5), Value::Number(n) if n.is_nan()));
        assert_eq!(c.valor(6), Value::Bool(true));
        assert_eq!(c.valor(8), Value::Empty, "fora da coluna");
    }

    /// A mesma dinâmica pela conta de fundo e pela conta na tela.
    #[test]
    fn conta_de_fundo_da_o_mesmo_resultado_da_conta_na_tela() {
        use feathersheet_core::dinamica::pivot::AgregacaoOp;
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            for (c, t) in ["Ramal", "Nome", "Ini", "Nivel", "Evento"].iter().enumerate() { s.set_cell(0, c as u32, t); }
            for r in 1..=500u32 {
                s.set_cell(r, 0, &format!("R{}", r % 3));
                s.set_cell(r, 1, &format!("N{}", r % 7));
                s.set_cell(r, 2, &(r % 4).to_string());
                if r % 11 != 0 { s.set_cell(r, 3, &format!("{}", (r * 37) % 101)); }
                s.set_cell(r, 4, if r % 2 == 0 { "JIC" } else { "PSUP" });
            }
        });
        app.fim_selecao = (500, 4);
        app.evento_dinamica(Evento::Abrir);
        app.evento_dinamica(Evento::Criar);
        let destino = app.folha;
        let mut e = app.dinamica.clone().expect("painel");
        e.linhas = vec![0, 1, 2];
        e.valores = vec![MedidaDinamica { coluna: 3, op: AgregacaoOp::DesvioPadrao }];
        e.filtros = vec![Filtro { coluna: 4, valores: ["PSUP".to_string()].into_iter().collect(), excluir: true }];
        app.aplicar_com_resultado(destino, &e, None).expect("na tela");
        let na_tela: Vec<Vec<String>> = (0..40).map(|r| (0..4).map(|c| app.workbook.aba(destino).unwrap().sheet.get_raw(r, c)).collect()).collect();
        // Pela conta de fundo, rodando a thread aqui mesmo.
        app.iniciar_dinamica_fundo(destino, &e).expect("começa");
        while !app.dinamica_fundo.trabalho.as_ref().unwrap().calculando {
            let _ = app.passo_dinamica_fundo();
        }
        let t = app.dinamica_fundo.trabalho.as_ref().unwrap();
        let colunas: HashMap<usize, Arc<ColunaLida>> = t.colunas.iter()
            .map(|c| (*c, Arc::clone(&app.dinamica_fundo.prontas[c]))).collect();
        assert!(t.colunas.contains(&4), "a coluna do filtro também é lida: {:?}", t.colunas);
        let cab = Arc::clone(&app.dinamica_fundo.cabecalhos);
        let leitor = |r: usize, c: usize| if r == 0 { cab[c].clone() } else { colunas.get(&c).map_or(Value::Empty, |col| col.valor(r - 1)) };
        let config = avancado::configuracao_avancada(&e).unwrap();
        let r = dinamica_layout::montar_com_extras(t.total, leitor, &config, &avancado::opcoes(&e), &avancado::extras_vigentes(&e));
        let geracao = t.geracao;
        app.dinamica_fundo_pronta(geracao, Pronto(Arc::new(Mutex::new(Some(r)))));
        let pelo_fundo: Vec<Vec<String>> = (0..40).map(|r| (0..4).map(|c| app.workbook.aba(destino).unwrap().sheet.get_raw(r, c)).collect()).collect();
        assert_eq!(pelo_fundo, na_tela);
        assert!(!app.dinamica_fundo.ativo());
    }

    /// Origem com coluna de fórmula sem conta: a dinâmica manda calcular em
    /// paralelo antes de copiar, em vez de calcular célula a célula na tela.
    #[test]
    fn origem_com_formula_pendente_calcula_em_fundo_antes_de_ler() {
        use feathersheet_core::dinamica::pivot::AgregacaoOp;
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0, 0, "Grupo"); s.set_cell(0, 1, "Valor"); s.set_cell(0, 2, "Dobro");
            for r in 1..=300u32 { s.set_cell(r, 0, &format!("G{}", r % 4)); s.set_cell(r, 1, &r.to_string()); }
            s.set_cell(1, 2, "=B2*2");
            assert!(s.preencher_formula_faixa_vazia(2, 2, 300, "=B3*2"));
        });
        assert!(app.workbook.aba(0).unwrap().sheet.formulas.tem_suja(2));
        app.fim_selecao = (300, 2);
        app.evento_dinamica(Evento::Abrir);
        app.evento_dinamica(Evento::Criar);
        let destino = app.folha;
        let mut e = app.dinamica.clone().expect("painel");
        e.linhas = vec![0];
        e.valores = vec![MedidaDinamica { coluna: 2, op: AgregacaoOp::Soma }];
        app.iniciar_dinamica_fundo(destino, &e).expect("começa");
        let _ = app.passo_dinamica_fundo();
        assert!(app.dinamica_fundo.trabalho.as_ref().unwrap().calculou_origem);
        assert!(app.status.contains("calculadas"), "{}", app.status);
        assert!(!app.workbook.aba(0).unwrap().sheet.formulas.tem_suja(2), "a origem voltou calculada");
        while !app.dinamica_fundo.trabalho.as_ref().unwrap().calculando {
            let _ = app.passo_dinamica_fundo();
        }
    }

}
