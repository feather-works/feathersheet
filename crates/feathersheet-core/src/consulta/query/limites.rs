//! Estimativa de cardinalidade ANTES de alocar o resultado de cada transformação.
//! O executor tradicional continua sem teto para consumidores não interativos.
use super::*;

/// Retorna erro se a próxima etapa ultrapassaria o orçamento do editor Iced.
pub(super) fn verificar(
    tabela: &Tabela,
    passo: &Passo,
    fontes: &HashMap<String, Tabela>,
    max_celulas: u64,
) -> Result<(), String> {
    // O executor TS tradicional tolera fonte ausente, mas um editor
    // interativo não deve salvar silenciosamente JOIN/APPEND incompleto.
    match passo {
        Passo::Juntar { fonte, esquerda, direita, modo } => {
            let outra = fontes.get(fonte).ok_or_else(|| format!(
                "A fonte '{fonte}' não está carregada. Selecione a tabela secundária."
            ))?;
            let (esq, dir) = (esquerda.to_vec(), direita.to_vec());
            if esq.is_empty() || esq.len() != dir.len() {
                return Err("A junção precisa da mesma quantidade de colunas-chave".into());
            }
            if esq.iter().any(|i| *i >= tabela.cabecalho.len()) ||
                dir.iter().any(|i| *i >= outra.cabecalho.len()) {
                return Err("Uma das colunas-chave da junção não existe mais".into());
            }
            if let Some(modo) = modo {
                if !["esquerda","interna","direita","total","anti_esquerda","anti_direita"]
                    .contains(&modo.as_str()) {
                    return Err(format!("Modo de junção '{modo}' desconhecido"));
                }
            }
        }
        Passo::Acrescentar { fonte } => {
            if !fontes.contains_key(fonte) {
                return Err(format!("A fonte '{fonte}' não está carregada para Acrescentar"));
            }
        }
        _ => {}
    }
    let mut linhas = tabela.linhas.len() as u64;
    let mut colunas = tabela.cabecalho.len() as u64;
    let operacao = match passo {
        Passo::Juntar { .. } => "Juntar",
        Passo::Pivotar { .. } => "Pivotar",
        Passo::Despivotar { .. } => "Despivotar",
        Passo::Dividir { .. } => "Dividir",
        Passo::Acrescentar { .. } => "Acrescentar",
        _ => "Transformar",
    };
    match passo {
        Passo::Colunas { manter } => colunas = manter.len() as u64,
        Passo::Mesclar { .. } | Passo::Derivar { .. } |
        Passo::Indice { .. } | Passo::Condicional { .. } => colunas += 1,
        Passo::Limite { quantas } => {
            linhas = linhas.min(linhas_do_limite(*quantas) as u64);
        }
        Passo::PromoverCabecalho => {
            if let Some(primeira) = tabela.linhas.first() {
                linhas = linhas.saturating_sub(1);
                colunas = primeira.len() as u64;
            }
        }
        Passo::RebaixarCabecalho => linhas = linhas.saturating_add(1),
        Passo::Dividir { col, separador } if !separador.is_empty() => {
            linhas = tabela.linhas.iter().map(|linha| match linha.get(*col) {
                Some(Value::Text(texto)) if texto.contains(separador.as_str()) =>
                    texto.split(separador.as_str()).count() as u64,
                _ => 1,
            }).fold(0_u64, |a, b| a.saturating_add(b));
        }
        Passo::DividirColunas { col, separador } if !separador.is_empty() => {
            let n = tabela.linhas.iter().map(|linha| match linha.get(*col) {
                Some(Value::Text(texto)) => texto.split(separador.as_str()).count() as u64,
                _ => 1,
            }).max().unwrap_or(1);
            colunas = colunas.saturating_add(n - 1);
        }
        Passo::Despivotar { manter, .. } => {
            let extra = (0..tabela.cabecalho.len())
                .filter(|i| !manter.contains(i)).count() as u64;
            linhas = linhas.saturating_mul(extra);
            colunas = manter.len() as u64 + 2;
        }
        Passo::Acrescentar { fonte } => {
            if let Some(outra) = fontes.get(fonte) {
                linhas = linhas.saturating_add(outra.linhas.len() as u64);
            }
        }
        Passo::Juntar { fonte, esquerda, direita, modo } => {
            if let Some(outra) = fontes.get(fonte) {
                colunas = colunas.saturating_add(outra.cabecalho.len() as u64);
                let (esq, dir) = (esquerda.to_vec(), direita.to_vec());
                let mut indice = HashMap::<String, u64>::new();
                for linha in &outra.linhas {
                    *indice.entry(chave_das(linha, &dir)).or_default() += 1;
                }
                let mut encontradas = HashSet::new();
                let (mut pares, mut so_esquerda) = (0_u64, 0_u64);
                for linha in &tabela.linhas {
                    let chave = chave_das(linha, &esq);
                    let quantidade = indice.get(&chave).copied().unwrap_or(0);
                    if quantidade == 0 {
                        so_esquerda += 1;
                    } else {
                        pares = pares.saturating_add(quantidade);
                        encontradas.insert(chave);
                    }
                }
                let so_direita = outra.linhas.iter()
                    .filter(|l| !encontradas.contains(&chave_das(l, &dir)))
                    .count() as u64;
                linhas = match modo.as_deref().unwrap_or("esquerda") {
                    "interna" => pares,
                    "direita" => pares.saturating_add(so_direita),
                    "total" => pares.saturating_add(so_esquerda).saturating_add(so_direita),
                    "anti_esquerda" => so_esquerda,
                    "anti_direita" => so_direita,
                    _ => pares.saturating_add(so_esquerda),
                };
            }
        }
        Passo::Agrupar { por, agregar } => {
            linhas = tabela.linhas.iter().map(|l| chave_das(l, por))
                .collect::<HashSet<_>>().len() as u64;
            colunas = (por.len() + agregar.len()) as u64;
        }
        Passo::Pivotar { linhas: eixo, colunas: campo, valores } => {
            let grupos = tabela.linhas.iter().map(|l| chave_das(l, eixo))
                .collect::<HashSet<_>>().len() as u64;
            let distintos = if campo.is_empty() { 0 } else {
                tabela.linhas.iter().map(|l| chave_das(l, campo))
                    .collect::<HashSet<_>>().len() as u64
            };
            linhas = grupos;
            colunas = (eixo.len() as u64).saturating_add(if campo.is_empty() {
                valores.len() as u64
            } else {
                distintos.saturating_mul(valores.len() as u64)
            });
        }
        _ => {}
    }
    // Mesmo quando o passo elimina todas as colunas, um Vec<Vec<Value>>
    // com milhões de linhas ainda consumiria memória.
    let estimativa = linhas.saturating_mul(colunas.max(1));
    if estimativa > max_celulas {
        Err(format!(
            "{operacao} exigiria {estimativa} células (máximo: {max_celulas}). \
             Reduza o intervalo ou as transformações antes de atualizar."
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn tabela(quantidade: usize, valores_iguais: bool) -> Tabela {
        Tabela {
            cabecalho: vec!["ID".into(), "Valor".into()],
            linhas: (0..quantidade).map(|i| vec![
                Value::Number(if valores_iguais { 1.0 } else { i as f64 }),
                Value::Text("teste".into()),
            ]).collect(),
        }
    }

    #[test]
    fn editor_recusa_fonte_inexistente_e_chaves_invalidas() {
        let t = tabela(2, false);
        let fontes = HashMap::<String,Tabela>::new();
        let passo = Passo::Acrescentar { fonte: "nao_existe".into() };
        assert!(verificar(&t,&passo,&fontes,10_000).unwrap_err().contains("não está carregada"));
        let passo = Passo::Juntar {
            fonte: "nao_existe".into(),
            esquerda: IndiceColuna::Single(0),
            direita: IndiceColuna::Single(0), modo: None,
        };
        assert!(verificar(&t,&passo,&fontes,10_000).is_err());
        let mut fontes = HashMap::new();
        fontes.insert("ok".into(),t.clone());
        let passo = Passo::Juntar {
            fonte:"ok".into(),esquerda:IndiceColuna::Multiple(vec![0,0]),
            direita:IndiceColuna::Single(0),modo:None,
        };
        assert!(verificar(&t,&passo,&fontes,10_000).is_err());
    }

    #[test]
    fn rejeita_produto_muitos_para_muitos_antes_da_juncao() {
        let fonte = tabela(1_000, true);
        let mut extras = HashMap::new();
        extras.insert("outra".into(), fonte.clone());
        let passo = Passo::Juntar {
            fonte: "outra".into(),
            esquerda: IndiceColuna::Single(0),
            direita: IndiceColuna::Single(0),
            modo: Some("interna".into()),
        };
        let erro = executar_query_limitado(fonte, &[passo], &extras, 100_000)
            .expect_err("join 1M x 4 colunas não pode ser materializado");
        assert!(erro.contains("Juntar"));
    }

    #[test]
    fn permite_juncao_um_para_um_com_cardinalidade_real() {
        let fonte = tabela(1_000, false);
        let mut extras = HashMap::new();
        extras.insert("outra".into(), fonte.clone());
        let passo = Passo::Juntar {
            fonte: "outra".into(),
            esquerda: IndiceColuna::Single(0),
            direita: IndiceColuna::Single(0),
            modo: Some("interna".into()),
        };
        let resultado = executar_query_limitado(fonte, &[passo], &extras, 8_000).unwrap();
        assert_eq!(resultado.linhas.len(), 1_000);
        assert_eq!(resultado.cabecalho.len(), 4);
    }

    #[test]
    fn despivotar_nao_cria_linhas_alem_do_orcamento() {
        let fonte = tabela(10_000, false);
        let passo = Passo::Despivotar {
            manter: vec![], nome_atributo: None, nome_valor: None,
        };
        let erro = executar_query_limitado(fonte, &[passo], &HashMap::new(), 39_999)
            .expect_err("o resultado 20K x 2 excede o limite");
        assert!(erro.contains("Despivotar"));
    }
}
