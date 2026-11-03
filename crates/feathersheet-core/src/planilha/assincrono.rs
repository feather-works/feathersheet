//! Valores que respondem depois: `IMPORTARDADOS`, `IMPORTARJSON`,
//! `BUSCARTEXTO` e as funções criadas por macro.
//!
//! Implementação nativa de `descarte/feathersheet/src/core/async.ts`;
//! com uma diferença de quem trabalha: lá a própria
//! aba disparava a busca (uma `Promise`); aqui o motor só anota a pergunta.
//! A célula mostra `#CALCULANDO!`, a interface pega as perguntas pendentes
//! (`tarefas_pendentes` na ponte), busca na rede ou roda a função da macro, e
//! devolve a resposta crua (`tarefa_resolver`). Quem pediu é recalculado.
//!
//! A chave descreve a pergunta inteira (função e argumentos): duas células
//! com a mesma pergunta dividem a mesma tarefa e a mesma resposta.

use std::collections::{HashMap, HashSet};

use crate::celulas::values::{CellError, Computed, ErrorCode, Matrix, Value};

/// Acima disto as respostas mais antigas saem (`MAX_ENTRIES`).
const MAX_RESPOSTAS: usize = 500;

/// A resposta de uma tarefa: um valor ou uma tabela para derramar.
#[derive(Clone, Debug, PartialEq)]
pub enum Resposta {
    Valor(Value),
    Matriz(Vec<Vec<Value>>),
}

impl Resposta {
    fn computado(&self) -> Computed {
        match self {
            Resposta::Valor(v) => Computed::Scalar(v.clone()),
            Resposta::Matriz(linhas) => Computed::Matrix(Matrix::new(linhas.clone())),
        }
    }

    /// A falha de uma tarefa vira `#ERROR!` com o motivo (`failure`).
    pub fn falha(motivo: impl Into<String>) -> Resposta {
        Resposta::Valor(Value::Error(CellError::com_motivo(ErrorCode::Error, motivo)))
    }
}

#[derive(Default)]
pub struct Assincronos {
    /// A resposta de cada chave e em que ordem chegou (para descartar a mais
    /// antiga).
    respondidas: HashMap<String, (Resposta, u64)>,
    chegada: u64,
    /// Perguntadas e ainda não entregues à interface, na ordem.
    pendentes: Vec<String>,
    /// As mesmas de `pendentes`, para conferir sem varrer a lista (50 mil fórmulas
    /// `IMPORTARDADOS` faziam 1,25 bilhão de comparações por recálculo).
    pendentes_set: HashSet<String>,
    /// Entregues à interface, esperando a resposta.
    em_curso: HashSet<String>,
    /// As células que fizeram cada pergunta, para recalcular quando chegar.
    quem_pede: HashMap<String, HashSet<u64>>,
}

impl Assincronos {
    /// O valor da pergunta, ou `#CALCULANDO!` enquanto ela não chega (a
    /// pergunta fica anotada para a interface buscar).
    pub fn pedir(&mut self, chave: &str, celula: u64) -> Computed {
        self.quem_pede.entry(chave.to_string()).or_default().insert(celula);
        if let Some((r, _)) = self.respondidas.get(chave) {
            return r.computado();
        }
        if !self.em_curso.contains(chave) && self.pendentes_set.insert(chave.to_string()) {
            self.pendentes.push(chave.to_string());
        }
        Computed::Scalar(Value::Error(CellError::new(ErrorCode::Calculating)))
    }

    /// As perguntas novas, que passam a estar "em curso".
    pub fn tomar_pendentes(&mut self) -> Vec<String> {
        let novas: Vec<String> = std::mem::take(&mut self.pendentes);
        self.pendentes_set.clear();
        self.em_curso.extend(novas.iter().cloned());
        novas
    }

    pub fn tem_pendentes(&self) -> bool {
        !self.pendentes.is_empty()
    }

    /// Guarda a resposta e devolve as células que a pediram.
    pub fn resolver(&mut self, chave: &str, resposta: Resposta) -> Vec<u64> {
        self.em_curso.remove(chave);
        if self.pendentes_set.remove(chave) { self.pendentes.retain(|p| p != chave); }
        self.chegada += 1;
        self.respondidas.insert(chave.to_string(), (resposta, self.chegada));
        self.descartar_antigas();
        // Igual ao AsyncValues.finish() TypeScript: entrega aos atuais,
        // zera os solicitantes, mas preserva a chave para o próximo cálculo.
        // Assim Atualizar consegue reencontrar células que leram do cache.
        self.quem_pede
            .get_mut(chave)
            .map(|solicitantes| solicitantes.drain().collect())
            .unwrap_or_default()
    }

    fn descartar_antigas(&mut self) {
        if self.respondidas.len() <= MAX_RESPOSTAS {
            return;
        }
        let mut por_idade: Vec<(String, u64)> = self.respondidas.iter().map(|(k, (_, t))| (k.clone(), *t)).collect();
        por_idade.sort_by_key(|(_, t)| *t);
        let sobra = por_idade.len() - MAX_RESPOSTAS;
        for (k, _) in por_idade.into_iter().take(sobra) {
            self.respondidas.remove(&k);
            // Não reter registros de URLs cujo valor expirou do cache.
            self.quem_pede.remove(&k);
        }
    }

    /// "Atualizar": esquece as respostas e devolve quem precisa perguntar de
    /// novo (`refresh`).
    pub fn atualizar(&mut self) -> Vec<u64> {
        let celulas: HashSet<u64> = self.quem_pede.values().flatten().copied().collect();
        self.respondidas.clear();
        self.quem_pede.clear();
        celulas.into_iter().collect()
    }

    /// O "atualizar" de um pedaço só: esquece as respostas das chaves que
    /// começam com `prefixo` e devolve quem as pediu. É o que acontece quando
    /// uma macro redefine a função (`macro:NOME` + NUL): o nome continua o
    /// mesmo, mas a resposta guardada era do corpo antigo — no TypeScript a
    /// função nova valia na hora, pelo `recalculateAll`.
    pub fn esquecer(&mut self, prefixo: &str) -> Vec<u64> {
        let chaves: Vec<String> = self.respondidas.keys().filter(|k| k.starts_with(prefixo)).cloned().collect();
        let mut celulas: HashSet<u64> = HashSet::new();
        for k in chaves {
            self.respondidas.remove(&k);
            if let Some(quem) = self.quem_pede.remove(&k) {
                celulas.extend(quem);
            }
        }
        celulas.into_iter().collect()
    }

    /// Quantas respostas estão guardadas (`size`).
    pub fn len(&self) -> usize {
        self.respondidas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.respondidas.is_empty()
    }

    pub fn limpar(&mut self) {
        *self = Assincronos::default();
    }

    /// As respostas guardadas, para o arquivo (`toJSON`), da mais antiga à
    /// mais nova — a ordem de chegada é o que decide quem sai primeiro quando
    /// o cache passa do teto, e ela precisa sobreviver ao arquivo. O erro não
    /// vai: na volta a tarefa tenta de novo.
    pub fn respondidas(&self) -> Vec<(&String, &Resposta, u64)> {
        let mut v: Vec<(&String, &Resposta, u64)> = self
            .respondidas
            .iter()
            .filter(|(_, (r, _))| !matches!(r, Resposta::Valor(Value::Error(_))))
            .map(|(k, (r, t))| (k, r, *t))
            .collect();
        v.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.cmp(b.0)));
        v
    }

    /// Carrega as respostas guardadas no arquivo (`loadJSON`), na ordem em
    /// que chegaram (a primeira é a mais antiga).
    pub fn carregar(&mut self, respostas: impl IntoIterator<Item = (String, Resposta)>) {
        // Arquivos antigos podem conter mais respostas do que o limite atual.
        // Preserve somente as últimas MAX_RESPOSTAS sem materializar todo o
        // histórico na memória durante a leitura.
        let mut ultimas = std::collections::VecDeque::with_capacity(MAX_RESPOSTAS);
        for entrada in respostas {
            if ultimas.len() == MAX_RESPOSTAS {
                ultimas.pop_front();
            }
            ultimas.push_back(entrada);
        }

        self.limpar();
        for (chave, resposta) in ultimas {
            self.chegada += 1;
            self.respondidas.insert(chave, (resposta, self.chegada));
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn refresh_preserva_dependencia_de_resposta_cacheada() {
        let mut a = Assincronos::default();
        let calculando = Computed::Scalar(Value::Error(CellError::new(ErrorCode::Calculating)));
        assert_eq!(a.pedir("web:texto:comum", 17), calculando);
        assert_eq!(a.pedir("web:texto:comum", 18), calculando);
        assert_eq!(a.tomar_pendentes(), vec!["web:texto:comum".to_string()]);
        let mut avisados = a.resolver(
            "web:texto:comum",
            Resposta::Valor(Value::Text("resposta".into())),
        );
        avisados.sort_unstable();
        assert_eq!(avisados, vec![17, 18]);
        // A resposta não gera notificações repetidas sem novo pedido.
        assert!(a.resolver(
            "web:texto:comum",
            Resposta::Valor(Value::Text("resposta".into())),
        ).is_empty());
        // O recálculo encontra a resposta guardada e registra dependência.
        assert_eq!(
            a.pedir("web:texto:comum", 18),
            Computed::Scalar(Value::Text("resposta".into())),
        );
        assert!(a.tomar_pendentes().is_empty());
        assert_eq!(a.atualizar(), vec![18]);
        assert_eq!(a.pedir("web:texto:comum", 18), calculando);
        assert_eq!(a.tomar_pendentes(), vec!["web:texto:comum".to_string()]);
    }

    #[test]
    fn evicao_nao_retem_solicitantes_de_chaves_antigas() {
        let mut a = Assincronos::default();
        for i in 0..=MAX_RESPOSTAS {
            let chave = format!("web:texto:{i}");
            a.pedir(&chave, i as u64);
            a.resolver(&chave, Resposta::Valor(Value::Number(i as f64)));
        }
        assert_eq!(a.len(), MAX_RESPOSTAS);
        assert_eq!(a.quem_pede.len(), MAX_RESPOSTAS);
        assert!(!a.respondidas.contains_key("web:texto:0"));
        assert!(!a.quem_pede.contains_key("web:texto:0"));
    }

    #[test]
    fn pergunta_calcula_responde_e_atualiza() {
        let mut a = Assincronos::default();
        let calculando = Computed::Scalar(Value::Error(CellError::new(ErrorCode::Calculating)));
        assert_eq!(a.pedir("web:texto:x", 1), calculando);
        assert_eq!(a.pedir("web:texto:x", 2), calculando);
        assert_eq!(a.tomar_pendentes(), vec!["web:texto:x".to_string()]);
        // Em curso: não volta para a lista.
        a.pedir("web:texto:x", 3);
        assert!(a.tomar_pendentes().is_empty());
        let mut quem = a.resolver("web:texto:x", Resposta::Valor(Value::Text("oi".into())));
        quem.sort_unstable();
        assert_eq!(quem, vec![1, 2, 3]);
        assert_eq!(a.pedir("web:texto:x", 1), Computed::Scalar(Value::Text("oi".into())));
        assert_eq!(a.atualizar(), vec![1]);
        assert_eq!(a.pedir("web:texto:x", 1), calculando);
    }

    #[test]
    fn o_teto_descarta_a_mais_antiga_e_o_erro_nao_vai_ao_arquivo() {
        let mut a = Assincronos::default();
        for i in 0..=MAX_RESPOSTAS {
            a.resolver(&format!("k{i}"), Resposta::Valor(Value::Number(i as f64)));
        }
        assert_eq!(a.len(), MAX_RESPOSTAS);
        // A primeira a chegar foi a que saiu.
        assert!(matches!(a.pedir("k0", 1), Computed::Scalar(Value::Error(_))));
        assert_eq!(a.pedir("k1", 1), Computed::Scalar(Value::Number(1.0)));

        let mut b = Assincronos::default();
        b.resolver("ruim", Resposta::falha("caiu"));
        b.resolver("bom", Resposta::Valor(Value::Text("x".into())));
        let guardadas: Vec<&String> = b.respondidas().into_iter().map(|(k, _, _)| k).collect();
        assert_eq!(guardadas, vec!["bom"]);
    }

    #[test]
    fn esquecer_so_o_prefixo() {
        let mut a = Assincronos::default();
        a.pedir("macro:DOBRO\u{0}[1]", 1);
        a.pedir("web:texto:u", 2);
        a.tomar_pendentes();
        a.resolver("macro:DOBRO\u{0}[1]", Resposta::Valor(Value::Number(2.0)));
        a.resolver("web:texto:u", Resposta::Valor(Value::Text("oi".into())));
        a.pedir("macro:DOBRO\u{0}[1]", 1);
        a.pedir("web:texto:u", 2);
        assert_eq!(a.esquecer("macro:DOBRO\u{0}"), vec![1]);
        assert_eq!(a.len(), 1);
        assert_eq!(a.pedir("web:texto:u", 2), Computed::Scalar(Value::Text("oi".into())));
    }

    #[test]
    fn restaurar_cache_respeita_limite_sem_reter_historico_inteiro() {
        let mut a = Assincronos::default();
        a.carregar((0..2_000).map(|i| {
            (format!("url:{i}"), Resposta::Valor(Value::Number(i as f64)))
        }));

        assert_eq!(a.len(), MAX_RESPOSTAS);
        assert_eq!(a.pedir("url:1999", 1), Computed::Scalar(Value::Number(1999.0)));
        assert_eq!(a.pedir("url:1500", 2), Computed::Scalar(Value::Number(1500.0)));
        assert!(matches!(
            a.pedir("url:1499", 3),
            Computed::Scalar(Value::Error(_))
        ));
        assert_eq!(a.tomar_pendentes(), vec!["url:1499".to_string()]);
    }
}
