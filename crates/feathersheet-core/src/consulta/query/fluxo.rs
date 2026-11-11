//! Executor incremental do FeatherQuery para transformações independentes por
//! linha. Mantém o pipeline em Rust, sem materializar todas as linhas.
//! Operações que precisam de estado global são recusadas explicitamente.
use super::*;

fn suporta(p: &Passo) -> bool {
    matches!(p,
        Passo::Filtrar { .. } | Passo::Limite { .. } |
        Passo::Renomear { .. } | Passo::Colunas { .. } |
        Passo::Tipar { .. } | Passo::Substituir { .. } |
        Passo::SubstituirErros { .. } | Passo::RemoverErros { .. } |
        Passo::Mesclar { .. } | Passo::Derivar { .. } |
        Passo::Dividir { .. } | Passo::Condicional { .. } |
        Passo::Indice { .. }
    )
}

/// Aplica passos compatíveis com streaming, sem carregar a matriz inteira.
/// A fonte fornece UMA linha de cada vez; o destino recebe cada linha pronta.
/// O callback de saída pode escrever diretamente em armazenamento colunar.
///
/// `limite_linhas` é um teto, não um truncamento silencioso: se o resultado
/// excedê-lo, a operação retorna erro. Destinos devem gravar numa transação
/// descartável e confirmar somente ao receber Ok.
///
/// Reutiliza `aplicar_passo` para preservar a semântica TypeScript por linha;
/// limites e índice mantêm contadores globais entre linhas.
pub fn executar_query_em_fluxo(
    cabecalho: Vec<String>,
    passos: &[Passo],
    mut proxima: impl FnMut() -> Option<Vec<Value>>,
    mut escrever: impl FnMut(&[Value]) -> Result<(), String>,
    limite_linhas: usize,
) -> Result<(Vec<String>, usize), String> {
    for passo in passos {
        if !suporta(passo) {
            return Err(format!(
                "A etapa '{}' exige processamento global; use o executor vetorial",
                serde_tipo(passo),
            ));
        }
    }
    let fontes = HashMap::new();
    let mut cabecalhos = vec![cabecalho];
    // Calcular o schema de cada etapa uma vez, e não por linha.
    for passo in passos {
        let vazio = Tabela {
            cabecalho: cabecalhos.last().expect("schema inicial").clone(),
            linhas: Vec::new(),
        };
        let convertido = aplicar_passo(vazio, passo, &fontes);
        cabecalhos.push(convertido.cabecalho);
    }
    let mut contadores_limite = vec![0usize; passos.len()];
    let mut contadores_indice: Vec<f64> = passos.iter()
        .map(|p| match p {
            Passo::Indice { inicio, .. } => inicio.unwrap_or(0.0),
            _ => 0.0,
        }).collect();
    let mut total = 0usize;
    while let Some(linha) = proxima() {
        let mut linhas = vec![linha];
        for (i, passo) in passos.iter().enumerate() {
            if linhas.is_empty() { break; }
            match passo {
                Passo::Limite { quantas } => {
                    let restantes = linhas_do_limite(*quantas)
                        .saturating_sub(contadores_limite[i]);
                    if linhas.len() > restantes { linhas.truncate(restantes); }
                    contadores_limite[i] += linhas.len();
                }
                Passo::Indice { incremento, .. } => {
                    let incremento = incremento.unwrap_or(1.0);
                    for linha in &mut linhas {
                        linha.push(Value::Number(contadores_indice[i]));
                        contadores_indice[i] += incremento;
                    }
                }
                _ => {
                    let mut proximas = Vec::new();
                    for linha in linhas {
                        let entrada = Tabela {
                            cabecalho: cabecalhos[i].clone(),
                            linhas: vec![linha],
                        };
                        // Impede expansão de uma única linha antes que
                        // dividir/despivotar possa alocar um vetor enorme.
                        limites::verificar(
                            &entrada, passo, &fontes,
                            (limite_linhas.saturating_sub(total).max(1) as u64)
                                .saturating_mul(cabecalhos[i+1].len().max(1) as u64),
                        )?;
                        let saida = aplicar_passo(entrada, passo, &fontes);
                        if saida.linhas.len() > limite_linhas.saturating_sub(total)
                            .saturating_sub(proximas.len()) {
                            return Err(crate::tr!("FeatherQuery incremental excede o limite de saída").into());
                        }
                        proximas.extend(saida.linhas);
                    }
                    linhas = proximas;
                }
            }
        }
        for linha in linhas {
            if total >= limite_linhas {
                return Err(crate::tr!("FeatherQuery incremental excede o limite de linhas").into());
            }
            escrever(&linha)?;
            total += 1;
        }
        // Nenhuma linha posterior passa de uma etapa Limite já esgotada.
        if passos.iter().enumerate().any(|(i, p)| match p {
            Passo::Limite { quantas } =>
                contadores_limite[i] >= linhas_do_limite(*quantas),
            _ => false,
        }) {
            break;
        }
    }
    Ok((cabecalhos.pop().unwrap_or_default(), total))
}

fn serde_tipo(p: &Passo) -> &'static str {
    match p {
        Passo::Ordenar { .. } => "Ordenar",
        Passo::Distintas { .. } => "Distintas",
        Passo::Agrupar { .. } => "Agrupar",
        Passo::Pivotar { .. } => "Pivotar",
        Passo::Juntar { .. } => "Juntar",
        Passo::Acrescentar { .. } => "Acrescentar",
        Passo::Despivotar { .. } => "Despivotar",
        Passo::PromoverCabecalho => "Promover cabeçalho",
        Passo::RebaixarCabecalho => "Rebaixar cabeçalho",
        _ => "Transformar",
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn base() -> Tabela {
        Tabela {
            cabecalho: vec!["Grupo".into(), "MWh".into()],
            linhas: (0..40).map(|i| vec![
                Value::Text(format!("G{}",i%3)),
                Value::Number(i as f64),
            ]).collect(),
        }
    }

    fn comparar(passos: Vec<Passo>) {
        let t = base();
        let esperado = executar_query(t.clone(), &passos, &HashMap::new());
        let mut entrada = t.linhas.into_iter();
        let mut linhas = Vec::new();
        let (cabecalho, quantas) = executar_query_em_fluxo(
            t.cabecalho, &passos, || entrada.next(),
            |linha| { linhas.push(linha.to_vec()); Ok(()) }, 500,
        ).unwrap();
        assert_eq!(cabecalho, esperado.cabecalho);
        assert_eq!(linhas, esperado.linhas);
        assert_eq!(quantas, esperado.linhas.len());
    }

    #[test]
    fn paridade_filtrar_renomear_limitar_e_criar_indice() {
        comparar(vec![
            Passo::Filtrar { col: 0, criterio: Value::Text("G1".into()) },
            Passo::Renomear { col: 1, nome: "Consumo".into() },
            Passo::Indice { inicio: Some(10.), incremento: Some(2.), nome: None },
            Passo::Limite { quantas: 5.0 },
        ]);
    }

    #[test]
    fn paridade_dividir_e_limitar_em_ordem_variavel() {
        let t = Tabela {
            cabecalho: vec!["Valores".into()],
            linhas: vec![
                vec![Value::Text("A;B;C".into())],
                vec![Value::Text("D;E".into())],
            ],
        };
        for passos in [
            vec![Passo::Dividir { col:0, separador:";".into() },
                Passo::Limite { quantas:4.0 }],
            vec![Passo::Limite { quantas:1.0 },
                Passo::Dividir { col:0, separador:";".into() }],
        ] {
            let esperado = executar_query(t.clone(), &passos, &HashMap::new());
            let mut entrada = t.linhas.clone().into_iter();
            let mut linhas = Vec::new();
            let (cabecalho,_) = executar_query_em_fluxo(
                t.cabecalho.clone(),&passos,||entrada.next(),
                |linha| {linhas.push(linha.to_vec());Ok(())},100,
            ).unwrap();
            assert_eq!(cabecalho,esperado.cabecalho);
            assert_eq!(linhas,esperado.linhas);
        }
    }

    #[test]
    fn grande_numero_de_linhas_nao_materializa_tabela() {
        let mut lida = 0usize;
        let mut emitida = 0usize;
        let (cabecalho,total) = executar_query_em_fluxo(
            vec!["X".into()],
            &[Passo::Limite { quantas: 10.0 }],
            || {if lida >= 1_000_000 { None } else {
                lida+=1;Some(vec![Value::Number(lida as f64)])
            }},
            |linha| { assert_eq!(linha.len(),1);emitida+=1;Ok(()) },
            100,
        ).unwrap();
        assert_eq!(cabecalho,vec!["X"]);
        assert_eq!(total,10);
        assert_eq!(emitida,10);
        assert_eq!(lida,10, "um limite evita materializar o milhão de linhas");
    }

    #[test]
    fn etapas_globais_rejeitadas_antes_de_ler() {
        let mut tocou = false;
        let erro = executar_query_em_fluxo(
            vec!["A".into()],
            &[Passo::Ordenar { por:vec![] }],
            || {tocou=true;None},
            |_|Ok(()),100,
        ).unwrap_err();
        assert!(erro.contains("processamento global"));
        assert!(!tocou);
    }
}
