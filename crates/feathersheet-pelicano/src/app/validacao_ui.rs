//! Estado e conversão do formulário de validação de dados TS para o motor Rust.
//! Mantém a construção de regras independente do widget Iced.
use feathersheet_core::celulas::refs::{parse_range_ref, Rect};
use feathersheet_core::celulas::values::Value;
use feathersheet_core::operacoes::validation::{
    parse_hora_segundos, AlertaErro, DataOp, MensagensValidacao, NumeroOp, QuandoValidacao, Regra,
    TextoOp, Validacao,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Criterio {
    #[default] Numero,
    Data,
    /// Hora do dia (`HH:MM` ou `HH:MM:SS`), com os operadores do número.
    Hora,
    Texto,
    Lista,
    Intervalo,
    Caixa,
}

#[derive(Debug, Clone)]
pub struct Formulario {
    pub criterio: Criterio,
    pub operador: String,
    pub primeiro: String,
    pub segundo: String,
    /// Texto do alerta de erro.
    pub mensagem: String,
    /// Estilo do alerta: Parar (`Recusar`), Aviso (`Avisar`) ou Informações (`Informar`).
    pub estilo: QuandoValidacao,
    /// "Número inteiro" do Excel (vem do .xlsx): guardado para não se perder.
    pub inteiro: bool,
    /// Aba "Mensagem de entrada": título e texto mostrados ao selecionar a célula.
    pub entrada_titulo: String,
    pub entrada_texto: String,
    /// Título da janela do alerta de erro.
    pub erro_titulo: String,
}

/// Como no Excel, o alerta padrão de uma regra nova é Parar.
impl Default for Formulario {
    fn default() -> Self {
        Self {
            criterio: Criterio::default(),
            operador: String::new(),
            primeiro: String::new(),
            segundo: String::new(),
            mensagem: String::new(),
            estilo: QuandoValidacao::Recusar,
            inteiro: false,
            entrada_titulo: String::new(),
            entrada_texto: String::new(),
            erro_titulo: String::new(),
        }
    }
}

/// Limites de tamanho do Excel para os textos da regra.
pub const MAX_TITULO: usize = 32;
pub const MAX_TEXTO_ENTRADA: usize = 255;
pub const MAX_TEXTO_ERRO: usize = 225;

fn nome_op_numero(op: NumeroOp) -> &'static str {
    match op {
        NumeroOp::Entre => "entre", NumeroOp::Maior => "maior", NumeroOp::Menor => "menor",
        NumeroOp::Igual => "igual", NumeroOp::Fora => "fora", NumeroOp::Diferente => "diferente",
        NumeroOp::MaiorIgual => "maiorigual", NumeroOp::MenorIgual => "menorigual",
    }
}

fn op_numero(nome: &str) -> NumeroOp {
    match nome {
        "maior" => NumeroOp::Maior, "menor" => NumeroOp::Menor,
        "igual" => NumeroOp::Igual, "fora" => NumeroOp::Fora,
        "diferente" => NumeroOp::Diferente, "maiorigual" => NumeroOp::MaiorIgual,
        "menorigual" => NumeroOp::MenorIgual, _ => NumeroOp::Entre,
    }
}

fn texto_opcional(t: &str) -> Option<String> {
    (!t.trim().is_empty()).then(|| t.to_owned())
}

impl Formulario {
    pub fn carregar(validacao: &Validacao) -> Self {
        let m = &validacao.mensagens;
        let mut f = Self {
            mensagem: validacao.mensagem.clone().unwrap_or_default(),
            estilo: validacao.quando,
            entrada_titulo: m.entrada_titulo.clone().unwrap_or_default(),
            entrada_texto: m.entrada_texto.clone().unwrap_or_default(),
            erro_titulo: m.erro_titulo.clone().unwrap_or_default(),
            ..Default::default()
        };
        match &validacao.regra {
            Regra::Numero { op, a, b, inteiro } => {
                f.criterio = Criterio::Numero;
                f.inteiro = *inteiro;
                f.operador = nome_op_numero(*op).into();
                f.primeiro = a.to_string();
                f.segundo = b.map(|n| n.to_string()).unwrap_or_default();
            }
            Regra::Data { op, a, b } => {
                f.criterio = Criterio::Data;
                f.operador = match op { DataOp::Entre => "entre", DataOp::Antes => "antes",
                    DataOp::Depois => "depois" }.into();
                f.primeiro = a.clone();
                f.segundo = b.clone().unwrap_or_default();
            }
            Regra::Texto { op, valor } => {
                f.criterio = Criterio::Texto;
                f.operador = match op { TextoOp::Contem => "contem", TextoOp::Comeca => "comeca",
                    TextoOp::Email => "email", TextoOp::Url => "url" }.into();
                f.primeiro = valor.clone().unwrap_or_default();
            }
            Regra::Lista { valores } => {
                f.criterio = Criterio::Lista;
                f.primeiro = valores.join("; ");
            }
            Regra::Intervalo { origem } => {
                f.criterio = Criterio::Intervalo;
                f.primeiro = format!("{}:{}",
                    feathersheet_core::celulas::refs::cell_name(origem.r0, origem.c0),
                    feathersheet_core::celulas::refs::cell_name(origem.r1, origem.c1));
            }
            // Nome, outra aba ou INDIRETO: o mesmo campo da origem do intervalo.
            Regra::Formula { formula } => {
                f.criterio = Criterio::Intervalo;
                f.primeiro = formula.clone();
            }
            Regra::Hora { op, a, b } => {
                f.criterio = Criterio::Hora;
                f.operador = nome_op_numero(*op).into();
                f.primeiro = a.clone();
                f.segundo = b.clone().unwrap_or_default();
            }
            Regra::Caixa { marcado, desmarcado } => {
                f.criterio = Criterio::Caixa;
                f.primeiro = marcado.clone();
                f.segundo = desmarcado.clone();
            }
        }
        f
    }

    pub fn construir(&self, ranges: Vec<Rect>) -> Result<Validacao, String> {
        if ranges.is_empty() { return Err(tr!("Selecione um intervalo").into()); }
        let regra = self.construir_regra()?;
        for (rotulo, texto, max) in [
            (tr!("O título da mensagem de entrada"), &self.entrada_titulo, MAX_TITULO),
            (tr!("A mensagem de entrada"), &self.entrada_texto, MAX_TEXTO_ENTRADA),
            (tr!("O título do alerta de erro"), &self.erro_titulo, MAX_TITULO),
            (tr!("A mensagem de erro"), &self.mensagem, MAX_TEXTO_ERRO),
        ] {
            if texto.chars().count() > max {
                return Err(trf!("{rotulo} pode ter até {max} caracteres"));
            }
        }
        Ok(Validacao {
            ranges,
            regra,
            quando: self.estilo,
            mensagem: texto_opcional(&self.mensagem),
            mensagens: MensagensValidacao {
                entrada_titulo: texto_opcional(&self.entrada_titulo),
                entrada_texto: texto_opcional(&self.entrada_texto),
                erro_titulo: texto_opcional(&self.erro_titulo),
            },
        })
    }

    /// Só a regra (aba Configurações); as mensagens ficam em [`Self::construir`].
    fn construir_regra(&self) -> Result<Regra, String> {
        let numero = |s: &str| -> Result<f64, String> {
            let n = s.trim().replace(',', ".").parse::<f64>()
                .map_err(|_| tr!("Digite um número válido").to_string())?;
            if !n.is_finite() { return Err(tr!("O número deve ser finito").into()); }
            Ok(n)
        };
        let regra = match self.criterio {
            Criterio::Numero => {
                let op = op_numero(&self.operador);
                let a = numero(&self.primeiro)?;
                let b = if matches!(op, NumeroOp::Entre | NumeroOp::Fora) { Some(numero(&self.segundo)?) } else { None };
                Regra::Numero { op, a, b, inteiro: self.inteiro }
            }
            Criterio::Hora => {
                let op = op_numero(&self.operador);
                let hora = |s: &str| -> Result<String, String> {
                    let t = s.trim();
                    parse_hora_segundos(&Value::Text(t.to_owned()))
                        .map(|_| t.to_owned())
                        .ok_or_else(|| tr!("Digite a hora como HH:MM ou HH:MM:SS").to_string())
                };
                let a = hora(&self.primeiro)?;
                let b = if matches!(op, NumeroOp::Entre | NumeroOp::Fora) { Some(hora(&self.segundo)?) } else { None };
                Regra::Hora { op, a, b }
            }
            Criterio::Data => {
                let op = match self.operador.as_str() {
                    "antes" => DataOp::Antes, "depois" => DataOp::Depois, _ => DataOp::Entre,
                };
                if self.primeiro.trim().is_empty() ||
                    (op == DataOp::Entre && self.segundo.trim().is_empty()) {
                    return Err(tr!("Informe as datas obrigatórias").into());
                }
                Regra::Data { op, a: self.primeiro.trim().into(),
                    b: (op == DataOp::Entre).then(|| self.segundo.trim().into()) }
            }
            Criterio::Texto => {
                let op = match self.operador.as_str() {
                    "comeca" => TextoOp::Comeca, "email" => TextoOp::Email,
                    "url" => TextoOp::Url, _ => TextoOp::Contem,
                };
                if matches!(op, TextoOp::Contem | TextoOp::Comeca) && self.primeiro.is_empty() {
                    return Err(tr!("Informe o texto de comparação").into());
                }
                Regra::Texto { op, valor: if matches!(op, TextoOp::Email | TextoOp::Url) {
                    None
                } else { Some(self.primeiro.clone()) } }
            }
            // Como no Excel: "=A2:A9", "=Nome" ou "=INDIRETO(B1)" na lista é a origem dela.
            Criterio::Lista if self.primeiro.trim().starts_with('=') => {
                return Self { criterio: Criterio::Intervalo, ..self.clone() }.construir_regra();
            }
            Criterio::Lista => {
                let valores: Vec<_> = self.primeiro.split(['\n', ';', ',']).map(str::trim)
                    .filter(|s| !s.is_empty()).map(str::to_owned).collect();
                if valores.is_empty() { return Err(tr!("Digite pelo menos um item").into()); }
                Regra::Lista { valores }
            }
            Criterio::Intervalo => {
                let texto = self.primeiro.trim().trim_start_matches('=');
                if texto.is_empty() {
                    return Err(tr!("Informe a origem, como A2:A20, Dados!A2:A20, um nome ou INDIRETO(B1)").into());
                }
                // Outra aba, nome ou INDIRETO: lista por fórmula, resolvida pelo motor.
                let Some(origem) = parse_range_ref(texto).filter(|o| o.sheet.is_none()) else {
                    return Ok(Regra::Formula { formula: format!("={texto}") });
                };
                if origem.area() > 10_000 {
                    return Err(tr!("A origem da lista pode ter no máximo 10.000 células").into());
                }
                Regra::Intervalo { origem }
            }
            Criterio::Caixa => Regra::Caixa {
                marcado: if self.primeiro.is_empty() { "VERDADEIRO".into() }
                    else { self.primeiro.clone() },
                desmarcado: if self.segundo.is_empty() { "FALSO".into() }
                    else { self.segundo.clone() },
            },
        };
        Ok(regra)
    }

    /// Operadores oferecidos pelo critério, em pt-BR como no Excel. Vazio quando o
    /// critério não compara com operador (lista, intervalo, caixa).
    pub fn operadores(criterio: Criterio) -> &'static [Opcao] {
        match criterio {
            Criterio::Numero | Criterio::Hora => &OPERADORES_NUMERO,
            Criterio::Data => &OPERADORES_DATA,
            Criterio::Texto => &OPERADORES_TEXTO,
            _ => &[],
        }
    }

    /// O campo "Dados" só se aplica a critérios com operador.
    pub fn tem_operador(&self) -> bool {
        !Self::operadores(self.criterio).is_empty()
    }

    /// Valor de "Permitir" que corresponde ao critério atual (um `valor` de `PERMITIR`).
    pub fn permitir(&self) -> &'static str {
        match (self.criterio, self.inteiro) {
            (Criterio::Numero, true) => "inteiro",
            (Criterio::Numero, false) => "decimal",
            (Criterio::Data, _) => "data",
            (Criterio::Hora, _) => "hora",
            (Criterio::Texto, _) => "texto",
            (Criterio::Caixa, _) => "caixa",
            (Criterio::Intervalo, _) => "intervalo",
            (Criterio::Lista, _) => "lista",
        }
    }

    /// Troca o critério escolhido em "Permitir". Os campos digitados continuam;
    /// só o operador volta ao primeiro da lista nova, porque o anterior pode não existir nela.
    pub fn definir_permitir(&mut self, valor: &str) {
        let (criterio, inteiro) = match valor {
            "inteiro" => (Criterio::Numero, true),
            "decimal" => (Criterio::Numero, false),
            "data" => (Criterio::Data, false),
            "hora" => (Criterio::Hora, false),
            "texto" => (Criterio::Texto, false),
            "caixa" => (Criterio::Caixa, false),
            "intervalo" => (Criterio::Intervalo, false),
            _ => (Criterio::Lista, false),
        };
        if criterio != self.criterio {
            self.operador = Self::operadores(criterio).first()
                .map(|o| o.valor.to_owned()).unwrap_or_default();
        }
        self.criterio = criterio;
        self.inteiro = inteiro;
    }

    /// O segundo limite existe em "está entre" / "não está entre" e na caixa.
    pub fn usa_segundo(&self) -> bool {
        self.criterio == Criterio::Caixa ||
            (matches!(self.criterio, Criterio::Numero | Criterio::Data | Criterio::Hora) &&
                (self.operador.is_empty() || self.operador == "entre" || self.operador == "fora"))
    }

    /// E-mail e URL não pedem valor de comparação.
    pub fn usa_primeiro(&self) -> bool {
        !(self.criterio == Criterio::Texto && matches!(self.operador.as_str(), "email" | "url"))
    }

    /// Rótulos dos campos de valor conforme o critério e o operador: (primeiro, segundo).
    pub fn rotulos_valor(&self) -> (&'static str, &'static str) {
        match self.criterio {
            Criterio::Numero if self.usa_segundo() => (tr!("Mínimo"), tr!("Máximo")),
            Criterio::Data if self.usa_segundo() => (tr!("Data inicial"), tr!("Data final")),
            Criterio::Hora if self.usa_segundo() => (tr!("Hora inicial"), tr!("Hora final")),
            Criterio::Hora => (tr!("Hora"), ""),
            Criterio::Numero | Criterio::Texto => (tr!("Valor"), ""),
            Criterio::Data => (tr!("Data"), ""),
            Criterio::Lista => (tr!("Itens"), ""),
            Criterio::Intervalo => (tr!("Origem"), ""),
            Criterio::Caixa => (tr!("Valor marcado"), tr!("Valor desmarcado")),
        }
    }
}

/// Item de lista suspensa: `valor` é a chave estável; `rotulo`, o texto em pt-BR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opcao {
    pub valor: &'static str,
    pub rotulo: &'static str,
}

impl std::fmt::Display for Opcao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(tr!(self.rotulo))
    }
}

/// Lista "Permitir". "Número inteiro" e "Decimal" são o mesmo critério numérico,
/// separados pelo campo `inteiro` do formulário.
pub const PERMITIR: [Opcao; 8] = [
    Opcao { valor: "inteiro", rotulo: "Número inteiro" },
    Opcao { valor: "decimal", rotulo: "Decimal" },
    Opcao { valor: "lista", rotulo: "Lista" },
    Opcao { valor: "data", rotulo: "Data" },
    Opcao { valor: "hora", rotulo: "Hora" },
    Opcao { valor: "texto", rotulo: "Texto" },
    Opcao { valor: "caixa", rotulo: "Caixa de seleção" },
    Opcao { valor: "intervalo", rotulo: "Intervalo de células" },
];

const OPERADORES_NUMERO: [Opcao; 8] = [
    Opcao { valor: "entre", rotulo: "está entre" },
    Opcao { valor: "fora", rotulo: "não está entre" },
    Opcao { valor: "igual", rotulo: "igual a" },
    Opcao { valor: "diferente", rotulo: "diferente de" },
    Opcao { valor: "maior", rotulo: "maior que" },
    Opcao { valor: "menor", rotulo: "menor que" },
    Opcao { valor: "maiorigual", rotulo: "maior ou igual a" },
    Opcao { valor: "menorigual", rotulo: "menor ou igual a" },
];

const OPERADORES_DATA: [Opcao; 3] = [
    Opcao { valor: "entre", rotulo: "está entre" },
    Opcao { valor: "antes", rotulo: "antes de" },
    Opcao { valor: "depois", rotulo: "depois de" },
];

const OPERADORES_TEXTO: [Opcao; 4] = [
    Opcao { valor: "contem", rotulo: "contém" },
    Opcao { valor: "comeca", rotulo: "começa com" },
    Opcao { valor: "email", rotulo: "é um e-mail" },
    Opcao { valor: "url", rotulo: "é uma URL" },
];

/// Abas do diálogo, na ordem do Excel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AbaValidacao {
    #[default]
    Configuracoes,
    MensagemEntrada,
    AlertaErro,
}

/* A verificação fica fora do App: a edição simples e a colagem em bloco
   compartilham o mesmo comportamento, sem parsear o JSON a cada célula. */
pub fn validar_celula(
    workbook: &feathersheet_core::planilha::workbook::Workbook,
    folha: usize, linha: u32, coluna: u32,
    texto: &str, regras: &[Validacao],
) -> Result<Option<String>, String> {
    // Comparar lista/texto com a digitação literal ("001" não vira "1").
    // O motor de validação numérica já converte Value::Text para f64.
    let valor = if texto.is_empty() {
        Value::Empty
    } else {
        Value::Text(texto.to_owned())
    };
    let mut primeiro_aviso = None;
    for regra in regras.iter().filter(|v| v.ranges.iter()
        .any(|r| r.contains(linha, coluna)))
    {
        let resultado = workbook.validar_na_pasta(folha, linha, coluna, &valor, &regra.regra);
        if !resultado.ok {
            let texto = regra.mensagem.clone().or(resultado.motivo)
                .unwrap_or_else(|| tr!("Valor fora da regra de validação").into());
            if regra.quando == QuandoValidacao::Recusar {
                return Err(texto);
            }
            if primeiro_aviso.is_none() {
                primeiro_aviso = Some(texto);
            }
        }
    }
    Ok(primeiro_aviso)
}

/// O alerta de erro (Parar, Aviso ou Informações) que a digitação de
/// `texto` provoca na célula, ou `None` se o valor passa em todas as regras
/// que a cobrem. Parar tem prioridade: basta uma regra Parar violada.
pub fn alerta_da_celula(
    workbook: &feathersheet_core::planilha::workbook::Workbook,
    folha: usize, linha: u32, coluna: u32,
    texto: &str, regras: &[Validacao],
) -> Option<AlertaErro> {
    let valor = if texto.is_empty() { Value::Empty } else { Value::Text(texto.to_owned()) };
    let mut primeiro: Option<AlertaErro> = None;
    for regra in regras.iter().filter(|v| v.ranges.iter().any(|r| r.contains(linha, coluna))) {
        let resultado = workbook.validar_na_pasta(folha, linha, coluna, &valor, &regra.regra);
        if resultado.ok { continue; }
        let mut alerta = regra.alerta_de_erro(resultado.motivo.as_deref());
        // O motor usa o título padrão do Excel; aqui o nome é o da janela.
        if regra.mensagens.erro_titulo.as_deref().is_none_or(|t| t.trim().is_empty()) {
            alerta.titulo = tr!("Validação de dados").into();
        }
        if !alerta.permite_continuar { return Some(alerta); }
        primeiro.get_or_insert(alerta);
    }
    primeiro
}

/// Deserializa somente uma vez por operação; ausência de regras é lista vazia.
pub fn carregar_regras(json: Option<&str>) -> Result<Vec<Validacao>, String> {
    match json {
        Some(dados) if !dados.trim().is_empty() => serde_json::from_str(dados)
            .map_err(|_| tr!("As regras de validação da aba estão corrompidas").into()),
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn regras_recusar_bloqueiam_e_avisar_permite() {
        use feathersheet_core::planilha::workbook::Workbook;
        let wb = Workbook::new();
        let regra = Validacao {
            ranges: vec![Rect::cell(0, 0)],
            regra: Regra::Numero { op: NumeroOp::Maior, a: 5.0, b: None, inteiro: false },
            quando: QuandoValidacao::Recusar,
            mensagem: Some("Precisa ser maior que cinco".into()),
            mensagens: Default::default(),
        };
        assert_eq!(validar_celula(&wb, 0, 0, 0, "3", &[regra.clone()]),
            Err("Precisa ser maior que cinco".into()));
        assert_eq!(validar_celula(&wb, 0, 0, 1, "3", &[regra.clone()]), Ok(None));
        let avisar = Validacao { quando: QuandoValidacao::Avisar, ..regra };
        assert_eq!(validar_celula(&wb, 0, 0, 0, "3", &[avisar]),
            Ok(Some("Precisa ser maior que cinco".into())));
    }

    #[test]
    fn lista_de_texto_preserva_zero_a_esquerda() {
        use feathersheet_core::planilha::workbook::Workbook;
        let regra = Validacao {
            ranges: vec![Rect::cell(0, 0)],
            regra: Regra::Lista { valores: vec!["001".into(), "002".into()] },
            quando: QuandoValidacao::Recusar,
            mensagem: None,
            mensagens: Default::default(),
        };
        let wb = Workbook::new();
        assert!(validar_celula(&wb, 0, 0, 0, "001", &[regra.clone()]).is_ok());
        assert!(validar_celula(&wb, 0, 0, 0, "1", &[regra]).is_err());
    }

    #[test]
    fn intervalo_pode_consultar_formula_da_origem() {
        use feathersheet_core::planilha::workbook::Workbook;
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 1, 2, "=2+3".into());
        let regra = Validacao {
            ranges: vec![Rect::cell(0, 0)],
            regra: Regra::Intervalo { origem: Rect::cell(1, 2) },
            quando: QuandoValidacao::Recusar,
            mensagem: None,
            mensagens: Default::default(),
        };
        assert!(validar_celula(&wb, 0, 0, 0, "5", &[regra.clone()]).is_ok());
        assert!(validar_celula(&wb, 0, 0, 0, "6", &[regra]).is_err());
    }

    #[test]
    fn lista_ignora_linhas_vazias_e_preserva_mensagem() {
        let f = Formulario { criterio: Criterio::Lista, primeiro: "RJ\n\n ES ".into(),
            mensagem: "Estado inválido".into(), estilo: QuandoValidacao::Recusar, ..Default::default() };
        let v = f.construir(vec![Rect::new(0, 0, 10, 0)]).unwrap();
        assert_eq!(v.regra, Regra::Lista { valores: vec!["RJ".into(), "ES".into()] });
        assert_eq!(Formulario::carregar(&v).primeiro, "RJ; ES");
        assert_eq!(v.quando, QuandoValidacao::Recusar);
    }
    #[test]
    fn intervalo_de_origem_usa_referencias_a1() {
        let f = Formulario { criterio: Criterio::Intervalo, primeiro: "=$B$3:B7".into(),
            estilo: QuandoValidacao::Recusar, ..Default::default() };
        let v = f.construir(vec![Rect::cell(0, 0)]).unwrap();
        assert_eq!(v.regra, Regra::Intervalo { origem: Rect::new(2, 1, 6, 1) });
        assert_eq!(Formulario::carregar(&v).primeiro, "B3:B7");
        // O que não é intervalo desta aba vira lista por fórmula, resolvida pelo motor.
        let v = Formulario { primeiro: "Dados!A2:A9".into(), ..f.clone() }
            .construir(vec![Rect::cell(0, 0)]).unwrap();
        assert_eq!(v.regra, Regra::Formula { formula: "=Dados!A2:A9".into() });
        assert!(Formulario { primeiro: " = ".into(), ..f.clone() }
            .construir(vec![Rect::cell(0, 0)]).is_err());
    }

    #[test]
    fn lista_aceita_virgulas_e_ponto_e_virgula() {
        let f = Formulario { criterio: Criterio::Lista, primeiro: "Sim; Não, Talvez".into(),
            ..Default::default() };
        let v = f.construir(vec![Rect::cell(0, 0)]).unwrap();
        assert_eq!(v.regra, Regra::Lista {
            valores: vec!["Sim".into(), "Não".into(), "Talvez".into()]
        });
    }

    #[test]
    fn lista_com_igual_e_origem_como_no_excel() {
        let lista = |t: &str| Formulario { criterio: Criterio::Lista, primeiro: t.into(), ..Default::default() }
            .construir(vec![Rect::cell(0, 0)]).unwrap().regra;
        assert_eq!(lista("=INDIRETO(A1)"), Regra::Formula { formula: "=INDIRETO(A1)".into() });
        assert_eq!(lista(" =$B$3:$B$7"), Regra::Intervalo { origem: Rect::new(2, 1, 6, 1) });
        assert_eq!(lista("Sim; Não"), Regra::Lista { valores: vec!["Sim".into(), "Não".into()] });
    }

    #[test]
    fn numero_entre_exige_ambos_os_limites() {
        let f = Formulario { primeiro: "10".into(), ..Default::default() };
        assert!(f.construir(vec![Rect::new(0, 0, 0, 0)]).is_err());
    }

    #[test]
    fn trocar_permitir_nao_perde_os_campos_digitados() {
        let mut f = Formulario { criterio: Criterio::Numero, operador: "maior".into(),
            primeiro: "5".into(), segundo: "9".into(), mensagem: "Confira".into(),
            estilo: QuandoValidacao::Recusar, ..Default::default() };
        f.definir_permitir("data");
        assert_eq!(f.criterio, Criterio::Data);
        // O operador de número ("maior") não existe em data: volta ao primeiro da lista.
        assert_eq!(f.operador, "entre");
        assert_eq!((f.primeiro.as_str(), f.segundo.as_str(), f.mensagem.as_str(), f.estilo),
            ("5", "9", "Confira", QuandoValidacao::Recusar));
        f.definir_permitir("inteiro");
        assert!(f.inteiro && f.criterio == Criterio::Numero);
        assert_eq!(f.permitir(), "inteiro");
        f.definir_permitir("decimal");
        assert_eq!(f.permitir(), "decimal");
    }

    #[test]
    fn operador_e_segundo_limite_seguem_o_criterio() {
        let lista = Formulario { criterio: Criterio::Lista, ..Default::default() };
        assert!(!lista.tem_operador() && !lista.usa_segundo());
        let numero = Formulario { criterio: Criterio::Numero, operador: "entre".into(),
            ..Default::default() };
        assert!(numero.tem_operador() && numero.usa_segundo());
        assert_eq!(numero.rotulos_valor(), ("Mínimo", "Máximo"));
        let maior = Formulario { operador: "maior".into(), ..numero.clone() };
        assert!(!maior.usa_segundo());
        assert_eq!(maior.rotulos_valor(), ("Valor", ""));
        let email = Formulario { criterio: Criterio::Texto, operador: "email".into(),
            ..Default::default() };
        assert!(!email.usa_primeiro());
    }

    #[test]
    fn construir_preserva_mensagens_estilo_e_hora() {
        let f = Formulario { criterio: Criterio::Hora, operador: "entre".into(),
            primeiro: "08:00".into(), segundo: "18:30".into(),
            estilo: QuandoValidacao::Informar, mensagem: "Fora do expediente".into(),
            entrada_titulo: "Horário".into(), entrada_texto: "Entre 08:00 e 18:30".into(),
            erro_titulo: "Hora inválida".into(), ..Default::default() };
        let v = f.construir(vec![Rect::cell(0, 0)]).unwrap();
        assert_eq!(v.regra, Regra::Hora { op: NumeroOp::Entre, a: "08:00".into(), b: Some("18:30".into()) });
        assert_eq!(v.quando, QuandoValidacao::Informar);
        assert_eq!(v.mensagem_de_entrada(), Some(("Horário".into(), "Entre 08:00 e 18:30".into())));
        assert_eq!(v.mensagens.erro_titulo.as_deref(), Some("Hora inválida"));
        // Ida e volta pelo formulário não perde nada.
        let g = Formulario::carregar(&v);
        assert_eq!(g.construir(vec![Rect::cell(0, 0)]).unwrap(), v);
        assert_eq!(g.permitir(), "hora");
        // Hora mal escrita é recusada no formulário.
        assert!(Formulario { primeiro: "25:00".into(), ..f.clone() }.construir(vec![Rect::cell(0, 0)]).is_err());
        // Título acima do limite do Excel.
        assert!(Formulario { entrada_titulo: "x".repeat(33), ..f }.construir(vec![Rect::cell(0, 0)]).is_err());
    }

    #[test]
    fn regra_nova_comeca_com_alerta_parar() {
        assert_eq!(Formulario::default().estilo, QuandoValidacao::Recusar);
    }

    #[test]
    fn alerta_da_celula_segue_o_estilo() {
        use feathersheet_core::operacoes::validation::EstiloAlerta;
        use feathersheet_core::planilha::workbook::Workbook;
        let wb = Workbook::new();
        let regra = |quando| Validacao {
            ranges: vec![Rect::cell(0, 0)],
            regra: Regra::Numero { op: NumeroOp::Maior, a: 5.0, b: None, inteiro: false },
            quando, mensagem: None, mensagens: Default::default(),
        };
        let parar = alerta_da_celula(&wb, 0, 0, 0, "3", &[regra(QuandoValidacao::Recusar)]).unwrap();
        assert_eq!(parar.estilo, EstiloAlerta::Parar);
        assert!(!parar.permite_continuar);
        assert_eq!(parar.titulo, "Validação de dados");
        let aviso = alerta_da_celula(&wb, 0, 0, 0, "3", &[regra(QuandoValidacao::Avisar)]).unwrap();
        assert_eq!(aviso.estilo, EstiloAlerta::Aviso);
        let info = alerta_da_celula(&wb, 0, 0, 0, "3", &[regra(QuandoValidacao::Informar)]).unwrap();
        assert!(info.permite_continuar);
        assert!(alerta_da_celula(&wb, 0, 0, 0, "9", &[regra(QuandoValidacao::Recusar)]).is_none());
        // Parar vence o aviso de outra regra na mesma célula.
        let ambos = [regra(QuandoValidacao::Avisar), regra(QuandoValidacao::Recusar)];
        assert_eq!(alerta_da_celula(&wb, 0, 0, 0, "3", &ambos).unwrap().estilo, EstiloAlerta::Parar);
    }
}
