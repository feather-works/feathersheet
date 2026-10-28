//! O registro: que funções existem, como elas se chamam em português, e quais
//! delas o resto do motor precisa tratar de forma especial.
//!
//! Tradução de `src/core/functions.ts`.
//!
//! ## A diferença em relação ao TypeScript
//!
//! Lá, o `FUNCTIONS` era um objeto que crescia em tempo de carga: cada apelido
//! entrava nele apontando para a mesma implementação, e o `CANONICAL` era
//! preenchido no mesmo laço. Aqui as duas tabelas são **constantes** e a
//! resolução acontece na consulta: procura pelo nome como veio e, se não
//! achar, traduz o apelido e procura de novo. O resultado é o mesmo, sem
//! nada sendo montado na partida.

use crate::funcoes::fn_date;
use crate::funcoes::fn_db;
use crate::funcoes::fn_eng;
use crate::funcoes::fn_info;
use crate::funcoes::fn_lookup;
use crate::funcoes::fn_lookup_matrix;
use crate::funcoes::fn_math;
use crate::funcoes::fn_stats;
use crate::funcoes::fn_subtotal;
use crate::funcoes::fn_text;
use crate::funcoes::fn_complexo;
use crate::funcoes::fn_dist;
use crate::funcoes::fn_conversao;
use crate::funcoes::fn_extra;
use crate::funcoes::fn_fin;
use crate::funcoes::fn_titulos;
use crate::funcoes::regex;
use crate::funcoes::fn_referencia;
use crate::funcoes::fn_lambda;
use crate::funcoes::fn_regressao;
use crate::funcoes::fn_cupom;
use crate::funcoes::fn_compat;

pub use crate::funcoes::fn_helpers::FnImpl;

/// As funções implementadas, por nome canônico (em inglês).
static FUNCOES: &[(&str, FnImpl)] = &[
    ("SUM", fn_math::fn_sum),
    ("SUBTOTAL", fn_subtotal::fn_subtotal),
    ("AGGREGATE", fn_subtotal::fn_aggregate),
    ("SUMIF", fn_math::fn_sumif),
    ("SUMIFS", fn_math::fn_sumifs),
    ("SUMPRODUCT", fn_math::fn_sumproduct),
    ("ABS", fn_math::fn_abs),
    ("SQRT", fn_math::fn_sqrt),
    ("POWER", fn_math::fn_power),
    ("ROUND", fn_math::fn_round),
    ("INT", fn_math::fn_int),
    ("TRUNC", fn_math::fn_trunc),
    ("MOD", fn_math::fn_mod),
    ("PI", fn_math::fn_pi),
    ("PRODUCT", fn_math::fn_product),
    ("EXP", fn_math::fn_exp),
    ("LN", fn_math::fn_ln),
    ("LOG", fn_math::fn_log),
    ("LOG10", fn_math::fn_log10),
    ("ROUNDUP", fn_math::fn_roundup),
    ("ROUNDDOWN", fn_math::fn_rounddown),
    ("CEILING", fn_math::fn_ceiling),
    ("FLOOR", fn_math::fn_floor),
    ("SIN", fn_math::fn_sin),
    ("COS", fn_math::fn_cos),
    ("TAN", fn_math::fn_tan),
    ("RAND", fn_math::fn_rand),
    ("RANDBETWEEN", fn_math::fn_randbetween),
    ("SIGN", fn_math::fn_sign),
    ("ASIN", fn_math::fn_asin),
    ("ACOS", fn_math::fn_acos),
    ("ATAN", fn_math::fn_atan),
    ("ATAN2", fn_math::fn_atan2),
    ("SINH", fn_math::fn_sinh),
    ("COSH", fn_math::fn_cosh),
    ("TANH", fn_math::fn_tanh),
    ("DEGREES", fn_math::fn_degrees),
    ("RADIANS", fn_math::fn_radians),
    ("FACT", fn_math::fn_fact),
    ("COMBIN", fn_math::fn_combin),
    ("PERMUT", fn_math::fn_permut),
    // Datas e horas (fn_date.rs; ver docs/funcionalidades/FUNCOES.md).
    ("HOUR", fn_date::fn_hour),
    ("MINUTE", fn_date::fn_minute),
    ("SECOND", fn_date::fn_second),
    ("TIME", fn_date::fn_time),
    ("DATEVALUE", fn_date::fn_datevalue),
    ("TIMEVALUE", fn_date::fn_timevalue),
    ("DAYS", fn_date::fn_days),
    ("DAYS360", fn_date::fn_days360),
    ("ISOWEEKNUM", fn_date::fn_isoweeknum),
    ("YEARFRAC", fn_date::fn_yearfrac),
    ("NETWORKDAYS.INTL", fn_date::fn_networkdays_intl),
    ("WORKDAY.INTL", fn_date::fn_workday_intl),
    // Distribuições (fn_dist.rs) e nomes de compatibilidade.
    ("BETA.DIST", fn_dist::fn_beta_dist),
    ("BETA.INV", fn_dist::fn_beta_inv),
    ("BINOM.DIST", fn_dist::fn_binom_dist),
    ("BINOM.DIST.RANGE", fn_dist::fn_binom_dist_range),
    ("BINOM.INV", fn_dist::fn_binom_inv),
    ("CHISQ.DIST.RT", fn_dist::fn_chisq_dist_rt),
    ("CHISQ.INV", fn_dist::fn_chisq_inv),
    ("CHISQ.INV.RT", fn_dist::fn_chisq_inv_rt),
    ("EXPON.DIST", fn_dist::fn_expon_dist),
    ("F.DIST", fn_dist::fn_f_dist),
    ("F.DIST.RT", fn_dist::fn_f_dist_rt),
    ("F.INV", fn_dist::fn_f_inv),
    ("F.INV.RT", fn_dist::fn_f_inv_rt),
    ("GAMMA", fn_dist::fn_gamma),
    ("GAMMA.DIST", fn_dist::fn_gamma_dist),
    ("GAMMA.INV", fn_dist::fn_gamma_inv),
    ("GAMMALN", fn_dist::fn_gammaln),
    ("GAMMALN.PRECISE", fn_dist::fn_gammaln_precise),
    ("GAUSS", fn_dist::fn_gauss),
    ("HYPGEOM.DIST", fn_dist::fn_hypgeom_dist),
    ("LOGNORM.DIST", fn_dist::fn_lognorm_dist),
    ("LOGNORM.INV", fn_dist::fn_lognorm_inv),
    ("NEGBINOM.DIST", fn_dist::fn_negbinom_dist),
    ("NORM.S.INV", fn_dist::fn_norm_s_inv),
    ("PHI", fn_dist::fn_phi),
    ("POISSON.DIST", fn_dist::fn_poisson_dist),
    ("T.DIST.2T", fn_dist::fn_t_dist_2t),
    ("T.DIST.RT", fn_dist::fn_t_dist_rt),
    ("T.INV", fn_dist::fn_t_inv),
    ("T.INV.2T", fn_dist::fn_t_inv_2t),
    ("WEIBULL.DIST", fn_dist::fn_weibull_dist),
    ("CONFIDENCE.NORM", fn_dist::fn_confidence_norm),
    ("CONFIDENCE.T", fn_dist::fn_confidence_t),
    ("PERMUTATIONA", fn_dist::fn_permutationa),
    ("ERF", fn_dist::fn_erf),
    ("ERF.PRECISE", fn_dist::fn_erf_precise),
    ("ERFC", fn_dist::fn_erfc),
    ("ERFC.PRECISE", fn_dist::fn_erfc_precise),
    ("BETADIST", fn_dist::fn_betadist),
    ("HYPGEOMDIST", fn_dist::fn_hypgeomdist),
    ("LOGNORMDIST", fn_dist::fn_lognormdist),
    ("NEGBINOMDIST", fn_dist::fn_negbinomdist),
    ("NORMSDIST", fn_dist::fn_normsdist),
    ("TDIST", fn_dist::fn_tdist),
    ("BINOMDIST", fn_dist::fn_binom_dist),
    ("CHIDIST", fn_dist::fn_chisq_dist_rt),
    ("CHIINV", fn_dist::fn_chisq_inv_rt),
    ("CRITBINOM", fn_dist::fn_binom_inv),
    ("EXPONDIST", fn_dist::fn_expon_dist),
    ("FDIST", fn_dist::fn_f_dist_rt),
    ("FINV", fn_dist::fn_f_inv_rt),
    ("GAMMADIST", fn_dist::fn_gamma_dist),
    ("GAMMAINV", fn_dist::fn_gamma_inv),
    ("LOGINV", fn_dist::fn_lognorm_inv),
    ("NORMSINV", fn_dist::fn_norm_s_inv),
    ("POISSON", fn_dist::fn_poisson_dist),
    ("TINV", fn_dist::fn_t_inv_2t),
    ("WEIBULL", fn_dist::fn_weibull_dist),
    ("BETAINV", fn_dist::fn_beta_inv),
    ("CONFIDENCE", fn_dist::fn_confidence_norm),
    ("STDEVP", fn_stats::fn_stdev_p),
    ("VARP", fn_stats::fn_var_p),
    ("NORMDIST", fn_stats::fn_norm_dist),
    ("NORMINV", fn_stats::fn_norm_inv),
    ("PERCENTRANK", fn_stats::fn_percentrank_inc),
    ("EVEN", fn_math::fn_even),
    ("ODD", fn_math::fn_odd),
    ("GCD", fn_math::fn_gcd),
    ("LCM", fn_math::fn_lcm),
    ("MROUND", fn_math::fn_mround),
    ("QUOTIENT", fn_math::fn_quotient),
    ("SUMSQ", fn_math::fn_sumsq),
    ("SQRTPI", fn_math::fn_sqrtpi),
    ("FACTDOUBLE", fn_math::fn_factdouble),
    ("ROMAN", fn_math::fn_roman),
    ("ACOSH", fn_math::fn_acosh),
    ("ASINH", fn_math::fn_asinh),
    ("ATANH", fn_math::fn_atanh),
    ("ACOT", fn_math::fn_acot),
    ("ACOTH", fn_math::fn_acoth),
    ("COT", fn_math::fn_cot),
    ("COTH", fn_math::fn_coth),
    ("CSC", fn_math::fn_csc),
    ("CSCH", fn_math::fn_csch),
    ("SEC", fn_math::fn_sec),
    ("SECH", fn_math::fn_sech),
    ("BASE", fn_math::fn_base),
    ("DECIMAL", fn_math::fn_decimal),
    ("ARABIC", fn_math::fn_arabic),
    ("CEILING.MATH", fn_math::fn_ceiling_math),
    ("CEILING.PRECISE", fn_math::fn_ceiling_precise),
    ("ISO.CEILING", fn_math::fn_iso_ceiling),
    ("FLOOR.MATH", fn_math::fn_floor_math),
    ("FLOOR.PRECISE", fn_math::fn_floor_precise),
    ("COMBINA", fn_math::fn_combina),
    ("MULTINOMIAL", fn_math::fn_multinomial),
    ("MUNIT", fn_math::fn_munit),
    ("SERIESSUM", fn_math::fn_seriessum),
    ("SUMX2MY2", fn_math::fn_sumx2my2),
    ("SUMX2PY2", fn_math::fn_sumx2py2),
    ("SUMXMY2", fn_math::fn_sumxmy2),
    ("CHAR", fn_text::fn_char),
    ("CODE", fn_text::fn_code),
    ("CLEAN", fn_text::fn_clean),
    ("EXACT", fn_text::fn_exact),
    ("T", fn_text::fn_t),
    ("REPLACE", fn_text::fn_replace),
    ("UNICHAR", fn_text::fn_unichar),
    ("UNICODE", fn_text::fn_unicode),
    ("NUMBERVALUE", fn_text::fn_numbervalue),
    ("FIXED", fn_text::fn_fixed),
    ("DOLLAR", fn_text::fn_dollar),
    ("TEXTBEFORE", fn_text::fn_textbefore),
    ("TEXTAFTER", fn_text::fn_textafter),
    ("VALUETOTEXT", fn_text::fn_valuetotext),
    ("ARRAYTOTEXT", fn_text::fn_arraytotext),
    ("REGEXTEST", fn_text::fn_regextest),
    ("ISERR", fn_info::fn_iserr),
    ("ISNA", fn_info::fn_isna),
    ("ISLOGICAL", fn_info::fn_islogical),
    ("ISNONTEXT", fn_info::fn_isnontext),
    ("ERROR.TYPE", fn_info::fn_error_type),
    ("TYPE", fn_info::fn_type),
    ("N", fn_info::fn_n),
    // Engenharia: bases, bits e comparações (fn_eng.rs).
    ("BIN2DEC", fn_eng::fn_bin2dec),
    ("BIN2HEX", fn_eng::fn_bin2hex),
    ("BIN2OCT", fn_eng::fn_bin2oct),
    ("DEC2BIN", fn_eng::fn_dec2bin),
    ("DEC2HEX", fn_eng::fn_dec2hex),
    ("DEC2OCT", fn_eng::fn_dec2oct),
    ("HEX2BIN", fn_eng::fn_hex2bin),
    ("HEX2DEC", fn_eng::fn_hex2dec),
    ("HEX2OCT", fn_eng::fn_hex2oct),
    ("OCT2BIN", fn_eng::fn_oct2bin),
    ("OCT2DEC", fn_eng::fn_oct2dec),
    ("OCT2HEX", fn_eng::fn_oct2hex),
    ("BITAND", fn_eng::fn_bitand),
    ("BITOR", fn_eng::fn_bitor),
    ("BITXOR", fn_eng::fn_bitxor),
    ("BITLSHIFT", fn_eng::fn_bitlshift),
    ("BITRSHIFT", fn_eng::fn_bitrshift),
    ("DELTA", fn_eng::fn_delta),
    ("GESTEP", fn_eng::fn_gestep),

    ("AVERAGE", fn_stats::fn_average),
    ("AVERAGEIF", fn_stats::fn_averageif),
    ("AVERAGEIFS", fn_stats::fn_averageifs),
    ("MAX", fn_stats::fn_max),
    ("MAXIFS", fn_stats::fn_maxifs),
    ("MIN", fn_stats::fn_min),
    ("MINIFS", fn_stats::fn_minifs),
    ("COUNT", fn_stats::fn_count),
    ("COUNTA", fn_stats::fn_counta),
    ("COUNTIF", fn_stats::fn_countif),
    ("COUNTIFS", fn_stats::fn_countifs),
    ("COUNTBLANK", fn_stats::fn_countblank),
    ("STDEV.P", fn_stats::fn_stdev_p),
    ("STDEV.S", fn_stats::fn_stdev_s),
    ("STDEV", fn_stats::fn_stdev_s),
    ("VAR.P", fn_stats::fn_var_p),
    ("VAR.S", fn_stats::fn_var_s),
    ("VAR", fn_stats::fn_var_s),
    ("MODE.SNGL", fn_stats::fn_mode_sngl),
    ("MODE", fn_stats::fn_mode_sngl),
    ("PERCENTILE.INC", fn_stats::fn_percentile_inc),
    ("PERCENTILE", fn_stats::fn_percentile_inc),
    ("QUARTILE.INC", fn_stats::fn_quartile_inc),
    ("QUARTILE", fn_stats::fn_quartile_inc),
    ("MEDIAN", fn_stats::fn_median),
    ("LARGE", fn_stats::fn_large),
    ("SMALL", fn_stats::fn_small),
    ("RANK", fn_stats::fn_rank),
    ("CORREL", fn_stats::fn_correl),
    ("COVAR", fn_stats::fn_covar),
    ("SLOPE", fn_stats::fn_slope),
    ("INTERCEPT", fn_stats::fn_intercept),
    ("RSQ", fn_stats::fn_rsq),
    ("FORECAST", fn_stats::fn_forecast),
    ("GEOMEAN", fn_stats::fn_geomean),
    ("HARMEAN", fn_stats::fn_harmean),
    ("AVEDEV", fn_stats::fn_avedev),
    ("DEVSQ", fn_stats::fn_devsq),
    ("SKEW", fn_stats::fn_skew),
    ("KURT", fn_stats::fn_kurt),
    ("NORM.DIST", fn_stats::fn_norm_dist),
    ("NORM.S.DIST", fn_stats::fn_norm_s_dist),
    ("NORM.INV", fn_stats::fn_norm_inv),
    ("T.DIST", fn_stats::fn_t_dist),
    ("T.TEST", fn_stats::fn_t_test),
    ("CHISQ.DIST", fn_stats::fn_chisq_dist),
    ("CHISQ.TEST", fn_stats::fn_chisq_test),
    ("FREQUENCY", fn_stats::fn_frequency),

    ("TODAY", fn_date::fn_today),
    ("NOW", fn_date::fn_now),
    ("DATE", fn_date::fn_date),
    ("YEAR", fn_date::fn_year),
    ("MONTH", fn_date::fn_month),
    ("DAY", fn_date::fn_day),
    ("WEEKDAY", fn_date::fn_weekday),
    ("WEEKNUM", fn_date::fn_weeknum),
    ("DATEDIF", fn_date::fn_datedif),
    ("EDATE", fn_date::fn_edate),
    ("EOMONTH", fn_date::fn_eomonth),
    ("WORKDAY", fn_date::fn_workday),
    ("NETWORKDAYS", fn_date::fn_networkdays),

    ("UPPER", fn_text::fn_upper),
    ("IMAGE", fn_text::fn_image),
    ("IMAGE.BASE64", fn_text::fn_image_base64),
    ("LOWER", fn_text::fn_lower),
    ("LEN", fn_text::fn_len),
    ("CONCAT", fn_text::fn_concat),
    ("CONCATENATE", fn_text::fn_concatenate),
    ("LEFT", fn_text::fn_left),
    ("RIGHT", fn_text::fn_right),
    ("TRIM", fn_text::fn_trim),
    ("MID", fn_text::fn_mid),
    ("FIND", fn_text::fn_find),
    ("SEARCH", fn_text::fn_search),
    ("REPT", fn_text::fn_rept),
    ("SUBSTITUTE", fn_text::fn_substitute),
    ("PROPER", fn_text::fn_proper),
    ("VALUE", fn_text::fn_value),
    ("TEXT", fn_text::fn_text_fmt),
    ("TEXTJOIN", fn_text::fn_textjoin),
    ("TEXTSPLIT", fn_text::fn_textsplit),
    ("JOIN", fn_text::fn_join),
    ("COUNTUNIQUE", fn_stats::fn_countunique),
    ("SUMBYCOLOR", super::fn_cor::fn_sumbycolor),
    ("COUNTBYCOLOR", super::fn_cor::fn_countbycolor),

    ("REGEXMATCH", regex::fn_regexmatch),
    ("REGEXEXTRACT", regex::fn_regexextract),
    ("REGEXREPLACE", regex::fn_regexreplace),

    ("IF", fn_lookup::fn_if),
    ("IFERROR", fn_lookup::fn_iferror),
    ("IFNA", fn_info::fn_ifna),
    ("IFS", fn_info::fn_ifs),
    ("SWITCH", fn_info::fn_switch),
    ("CHOOSE", fn_info::fn_choose),
    ("AND", fn_lookup::fn_and),
    ("OR", fn_lookup::fn_or),
    ("XOR", fn_info::fn_xor),
    ("NOT", fn_lookup::fn_not),
    ("TRUE", fn_info::fn_true),
    ("FALSE", fn_info::fn_false),
    ("NA", fn_info::fn_na),
    ("ISBLANK", fn_info::fn_isblank),
    ("ISERROR", fn_info::fn_iserror),
    ("ISNUMBER", fn_info::fn_isnumber),
    ("ISTEXT", fn_info::fn_istext),
    ("ISEVEN", fn_info::fn_iseven),
    ("ISODD", fn_info::fn_isodd),
    ("ROW", fn_info::fn_row),
    ("COLUMN", fn_info::fn_column),
    ("ROWS", fn_info::fn_rows),
    ("COLUMNS", fn_info::fn_columns),
    ("VLOOKUP", fn_lookup::fn_vlookup),
    ("HLOOKUP", fn_lookup::fn_hlookup),
    ("XLOOKUP", fn_lookup::fn_xlookup),
    ("MATCH", fn_lookup::fn_match),
    ("INDEX", fn_lookup::fn_index),
    ("FILTER", fn_lookup_matrix::fn_filter),
    ("SORT", fn_lookup_matrix::fn_sort),
    ("UNIQUE", fn_lookup_matrix::fn_unique),
    ("SEQUENCE", fn_lookup_matrix::fn_sequence),
    ("TRANSPOSE", fn_lookup_matrix::fn_transpose),
    ("MMULT", fn_lookup_matrix::fn_mmult),
    ("MDETERM", fn_lookup_matrix::fn_mdeterm),
    ("MINVERSE", fn_lookup_matrix::fn_minverse),
    ("IMPORTDATA", fn_info::fn_importdata),
    ("IMPORTJSON", fn_info::fn_importjson),
    ("FETCHTEXT", fn_info::fn_fetchtext),
    // Financeiras (fn_fin.rs).
    ("PMT", fn_fin::fn_pmt),
    ("FV", fn_fin::fn_fv),
    ("PV", fn_fin::fn_pv),
    ("NPER", fn_fin::fn_nper),
    ("IPMT", fn_fin::fn_ipmt),
    ("PPMT", fn_fin::fn_ppmt),
    ("ISPMT", fn_fin::fn_ispmt),
    ("SLN", fn_fin::fn_sln),
    ("SYD", fn_fin::fn_syd),
    ("DB", fn_fin::fn_db),
    ("DDB", fn_fin::fn_ddb),
    ("VDB", fn_fin::fn_vdb),
    ("EFFECT", fn_fin::fn_effect),
    ("NOMINAL", fn_fin::fn_nominal),
    ("DOLLARDE", fn_fin::fn_dollarde),
    ("DOLLARFR", fn_fin::fn_dollarfr),
    ("RRI", fn_fin::fn_rri),
    ("PDURATION", fn_fin::fn_pduration),
    ("NPV", fn_fin::fn_npv),
    ("RATE", fn_fin::fn_rate),
    ("CUMIPMT", fn_fin::fn_cumipmt),
    ("CUMPRINC", fn_fin::fn_cumprinc),
    ("XNPV", fn_fin::fn_xnpv),
    ("XIRR", fn_fin::fn_xirr),
    ("IRR", fn_fin::fn_irr),
    ("MIRR", fn_fin::fn_mirr),
    ("FVSCHEDULE", fn_fin::fn_fvschedule),
    // Banco de dados (fn_db.rs).
    ("DSUM", fn_db::fn_dsum),
    ("DAVERAGE", fn_db::fn_daverage),
    ("DCOUNT", fn_db::fn_dcount),
    ("DCOUNTA", fn_db::fn_dcounta),
    ("DGET", fn_db::fn_dget),
    ("DMAX", fn_db::fn_dmax),
    ("DMIN", fn_db::fn_dmin),
    ("DPRODUCT", fn_db::fn_dproduct),
    ("DSTDEV", fn_db::fn_dstdev),
    ("DSTDEVP", fn_db::fn_dstdevp),
    ("DVAR", fn_db::fn_dvar),
    ("DVARP", fn_db::fn_dvarp),
    // Estatística (fn_stats.rs).
    ("AVERAGEA", fn_stats::fn_averagea),
    ("MAXA", fn_stats::fn_maxa),
    ("MINA", fn_stats::fn_mina),
    ("STDEVA", fn_stats::fn_stdeva),
    ("STDEVPA", fn_stats::fn_stdevpa),
    ("VARA", fn_stats::fn_vara),
    ("VARPA", fn_stats::fn_varpa),
    ("RANK.EQ", fn_stats::fn_rank_eq),
    ("RANK.AVG", fn_stats::fn_rank_avg),
    ("PERCENTILE.EXC", fn_stats::fn_percentile_exc),
    ("QUARTILE.EXC", fn_stats::fn_quartile_exc),
    ("PERCENTRANK.INC", fn_stats::fn_percentrank_inc),
    ("PERCENTRANK.EXC", fn_stats::fn_percentrank_exc),
    ("COVARIANCE.P", fn_stats::fn_covariance_p),
    ("COVARIANCE.S", fn_stats::fn_covariance_s),
    ("PEARSON", fn_stats::fn_pearson),
    ("STANDARDIZE", fn_stats::fn_standardize),
    ("TRIMMEAN", fn_stats::fn_trimmean),
    ("FORECAST.LINEAR", fn_stats::fn_forecast_linear),
    ("SKEW.P", fn_stats::fn_skew_p),
    ("MODE.MULT", fn_stats::fn_mode_mult),
    ("FISHER", fn_stats::fn_fisher),
    ("FISHERINV", fn_stats::fn_fisherinv),
    ("PERCENTOF", fn_math::fn_percentof),
    // Engenharia: números complexos (fn_complexo.rs).
    ("COMPLEX", fn_complexo::fn_complex),
    ("IMABS", fn_complexo::fn_imabs),
    ("IMAGINARY", fn_complexo::fn_imaginary),
    ("IMARGUMENT", fn_complexo::fn_imargument),
    ("IMCONJUGATE", fn_complexo::fn_imconjugate),
    ("IMCOS", fn_complexo::fn_imcos),
    ("IMCOSH", fn_complexo::fn_imcosh),
    ("IMCOT", fn_complexo::fn_imcot),
    ("IMCSC", fn_complexo::fn_imcsc),
    ("IMCSCH", fn_complexo::fn_imcsch),
    ("IMDIV", fn_complexo::fn_imdiv),
    ("IMEXP", fn_complexo::fn_imexp),
    ("IMLN", fn_complexo::fn_imln),
    ("IMLOG10", fn_complexo::fn_imlog10),
    ("IMLOG2", fn_complexo::fn_imlog2),
    ("IMPOWER", fn_complexo::fn_impower),
    ("IMPRODUCT", fn_complexo::fn_improduct),
    ("IMREAL", fn_complexo::fn_imreal),
    ("IMSEC", fn_complexo::fn_imsec),
    ("IMSECH", fn_complexo::fn_imsech),
    ("IMSIN", fn_complexo::fn_imsin),
    ("IMSINH", fn_complexo::fn_imsinh),
    ("IMSQRT", fn_complexo::fn_imsqrt),
    ("IMSUB", fn_complexo::fn_imsub),
    ("IMSUM", fn_complexo::fn_imsum),
    ("IMTAN", fn_complexo::fn_imtan),
    // Lote de 15/10/2026: matriz dinâmica, pesquisa, estatística, texto, engenharia e títulos.
    ("VSTACK", fn_extra::fn_vstack),
    ("HSTACK", fn_extra::fn_hstack),
    ("TAKE", fn_extra::fn_take),
    ("DROP", fn_extra::fn_drop),
    ("CHOOSECOLS", fn_extra::fn_choosecols),
    ("CHOOSEROWS", fn_extra::fn_chooserows),
    ("TOCOL", fn_extra::fn_tocol),
    ("TOROW", fn_extra::fn_torow),
    ("WRAPROWS", fn_extra::fn_wraprows),
    ("WRAPCOLS", fn_extra::fn_wrapcols),
    ("EXPAND", fn_extra::fn_expand),
    ("SORTBY", fn_extra::fn_sortby),
    ("RANDARRAY", fn_extra::fn_randarray),
    ("LOOKUP", fn_extra::fn_lookup),
    ("XMATCH", fn_extra::fn_xmatch),
    ("ADDRESS", fn_extra::fn_address),
    ("HYPERLINK", fn_extra::fn_hyperlink),
    ("ISREF", fn_extra::fn_isref),
    ("STEYX", fn_extra::fn_steyx),
    ("PROB", fn_extra::fn_prob),
    ("F.TEST", fn_extra::fn_f_test),
    ("Z.TEST", fn_extra::fn_z_test),
    ("TREND", fn_extra::fn_trend),
    ("GROWTH", fn_extra::fn_growth),
    ("LENB", fn_extra::fn_lenb),
    ("LEFTB", fn_extra::fn_leftb),
    ("RIGHTB", fn_extra::fn_rightb),
    ("MIDB", fn_extra::fn_midb),
    ("FINDB", fn_extra::fn_findb),
    ("SEARCHB", fn_extra::fn_searchb),
    ("REPLACEB", fn_extra::fn_replaceb),
    ("ENCODEURL", fn_extra::fn_encodeurl),
    ("CONVERT", fn_conversao::fn_convert),
    ("BESSELJ", fn_conversao::fn_besselj),
    ("BESSELI", fn_conversao::fn_besseli),
    ("DISC", fn_titulos::fn_disc),
    ("PRICEDISC", fn_titulos::fn_pricedisc),
    ("YIELDDISC", fn_titulos::fn_yielddisc),
    ("INTRATE", fn_titulos::fn_intrate),
    ("RECEIVED", fn_titulos::fn_received),
    ("TBILLPRICE", fn_titulos::fn_tbillprice),
    ("TBILLYIELD", fn_titulos::fn_tbillyield),
    ("TBILLEQ", fn_titulos::fn_tbilleq),
    ("PRICEMAT", fn_titulos::fn_pricemat),
    ("YIELDMAT", fn_titulos::fn_yieldmat),
    ("ACCRINTM", fn_titulos::fn_accrintm),
    // Lote de 18/10/2026: o que faltava da lista do Excel (FUNCOES-FALTANTES.md).
    ("OFFSET", fn_referencia::fn_offset),
    ("INDIRECT", fn_referencia::fn_indirect),
    ("TRIMRANGE", fn_referencia::fn_trimrange),
    ("AREAS", fn_referencia::fn_areas),
    ("FORMULATEXT", fn_referencia::fn_formulatext),
    ("ISFORMULA", fn_referencia::fn_isformula),
    ("SHEET", fn_referencia::fn_sheet),
    ("SHEETS", fn_referencia::fn_sheets),
    ("CELL", fn_referencia::fn_cell),
    ("INFO", fn_referencia::fn_info),
    ("GETPIVOTDATA", fn_referencia::fn_getpivotdata),
    ("BYROW", fn_lambda::fn_byrow),
    ("BYCOL", fn_lambda::fn_bycol),
    ("SCAN", fn_lambda::fn_scan),
    ("MAKEARRAY", fn_lambda::fn_makearray),
    ("ISOMITTED", fn_lambda::fn_isomitted),
    ("GROUPBY", fn_lambda::fn_groupby),
    ("PIVOTBY", fn_lambda::fn_pivotby),
    ("LINEST", fn_regressao::fn_linest),
    ("LOGEST", fn_regressao::fn_logest),
    ("FORECAST.ETS", fn_regressao::fn_forecast_ets),
    ("FORECAST.ETS.CONFINT", fn_regressao::fn_forecast_ets_confint),
    ("FORECAST.ETS.SEASONALITY", fn_regressao::fn_forecast_ets_seasonality),
    ("FORECAST.ETS.STAT", fn_regressao::fn_forecast_ets_stat),
    ("ACCRINT", fn_cupom::fn_accrint),
    ("AMORDEGRC", fn_cupom::fn_amordegrc),
    ("AMORLINC", fn_cupom::fn_amorlinc),
    ("COUPDAYBS", fn_cupom::fn_coupdaybs),
    ("COUPDAYS", fn_cupom::fn_coupdays),
    ("COUPDAYSNC", fn_cupom::fn_coupdaysnc),
    ("COUPNCD", fn_cupom::fn_coupncd),
    ("COUPNUM", fn_cupom::fn_coupnum),
    ("COUPPCD", fn_cupom::fn_couppcd),
    ("DURATION", fn_cupom::fn_duration),
    ("MDURATION", fn_cupom::fn_mduration),
    ("ODDFPRICE", fn_cupom::fn_oddfprice),
    ("ODDFYIELD", fn_cupom::fn_oddfyield),
    ("ODDLPRICE", fn_cupom::fn_oddlprice),
    ("ODDLYIELD", fn_cupom::fn_oddlyield),
    ("PRICE", fn_cupom::fn_price),
    ("YIELD", fn_cupom::fn_yield),
    ("BESSELK", fn_conversao::fn_besselk),
    ("BESSELY", fn_conversao::fn_bessely),
    ("ASC", fn_compat::fn_asc),
    ("DBCS", fn_compat::fn_dbcs),
    ("BAHTTEXT", fn_compat::fn_bahttext),
    ("PHONETIC", fn_compat::fn_phonetic),
    ("EUROCONVERT", fn_compat::fn_euroconvert),
    ("FILTERXML", fn_compat::fn_filterxml),
    ("WEBSERVICE", fn_info::fn_fetchtext),
    ("CUBEKPIMEMBER", fn_compat::fn_cubekpimember),
    ("CUBEMEMBER", fn_compat::fn_cubemember),
    ("CUBEMEMBERPROPERTY", fn_compat::fn_cubememberproperty),
    ("CUBERANKEDMEMBER", fn_compat::fn_cuberankedmember),
    ("CUBESET", fn_compat::fn_cubeset),
    ("CUBESETCOUNT", fn_compat::fn_cubesetcount),
    ("CUBEVALUE", fn_compat::fn_cubevalue),
    ("RTD", fn_compat::fn_rtd),
    ("CALL", fn_compat::fn_call),
    ("REGISTER.ID", fn_compat::fn_register_id),
    ("STOCKHISTORY", fn_compat::fn_stockhistory),
    ("TRANSLATE", fn_compat::fn_translate),
    ("DETECTLANGUAGE", fn_compat::fn_detectlanguage),
];


/// Apelido em português -> nome canônico em inglês.
///
/// Transcrita do `ALIASES` do `functions.ts`, com a mesma regra: as duas
/// grafias, com e sem acento, porque as duas são digitadas.
static ALIASES: &[(&str, &str)] = &[
    // Lote de 18/10/2026.
    ("DESLOC", "OFFSET"),
    ("INDIRETO", "INDIRECT"),
    ("AJUSTARINTERVALO", "TRIMRANGE"),
    ("ÁREAS", "AREAS"),
    ("FÓRMULATEXTO", "FORMULATEXT"),
    ("FORMULATEXTO", "FORMULATEXT"),
    ("ÉFÓRMULA", "ISFORMULA"),
    ("EFORMULA", "ISFORMULA"),
    ("ÉFORMULA", "ISFORMULA"),
    ("PLAN", "SHEET"),
    ("PLANS", "SHEETS"),
    ("CÉL", "CELL"),
    ("CEL", "CELL"),
    ("INFORMAÇÃO", "INFO"),
    ("INFORMACAO", "INFO"),
    ("INFODADOSTABELADINÂMICA", "GETPIVOTDATA"),
    ("INFODADOSTABELADINAMICA", "GETPIVOTDATA"),
    ("PORLINHA", "BYROW"),
    ("PORCOL", "BYCOL"),
    ("VARRER", "SCAN"),
    ("CRIARMATRIZ", "MAKEARRAY"),
    ("ÉOMITIDO", "ISOMITTED"),
    ("EOMITIDO", "ISOMITTED"),
    ("AGRUPARPOR", "GROUPBY"),
    ("PIVOTARPOR", "PIVOTBY"),
    ("PROJ.LIN", "LINEST"),
    ("PROJ.LOG", "LOGEST"),
    ("PREVISÃO.ETS", "FORECAST.ETS"),
    ("PREVISAO.ETS", "FORECAST.ETS"),
    ("PREVISÃO.ETS.CONFINT", "FORECAST.ETS.CONFINT"),
    ("PREVISAO.ETS.CONFINT", "FORECAST.ETS.CONFINT"),
    ("PREVISÃO.ETS.SAZONALIDADE", "FORECAST.ETS.SEASONALITY"),
    ("PREVISAO.ETS.SAZONALIDADE", "FORECAST.ETS.SEASONALITY"),
    ("PREVISÃO.ETS.STAT", "FORECAST.ETS.STAT"),
    ("PREVISAO.ETS.STAT", "FORECAST.ETS.STAT"),
    ("JUROSACUM", "ACCRINT"),
    ("CUPDIASINLIQ", "COUPDAYBS"),
    ("CUPDIAS", "COUPDAYS"),
    ("CUPDIASPRÓX", "COUPDAYSNC"),
    ("CUPDIASPROX", "COUPDAYSNC"),
    ("CUPDATAPRÓX", "COUPNCD"),
    ("CUPDATAPROX", "COUPNCD"),
    ("CUPNÚM", "COUPNUM"),
    ("CUPNUM", "COUPNUM"),
    ("CUPDATAANT", "COUPPCD"),
    ("DURAÇÃO", "DURATION"),
    ("DURACAO", "DURATION"),
    ("MDURAÇÃO", "MDURATION"),
    ("MDURACAO", "MDURATION"),
    ("PREÇOPRIMINC", "ODDFPRICE"),
    ("PRECOPRIMINC", "ODDFPRICE"),
    ("LUCROPRIMINC", "ODDFYIELD"),
    ("PREÇOÚLTINC", "ODDLPRICE"),
    ("PRECOULTINC", "ODDLPRICE"),
    ("LUCROÚLTINC", "ODDLYIELD"),
    ("LUCROULTINC", "ODDLYIELD"),
    ("PREÇO", "PRICE"),
    ("PRECO", "PRICE"),
    ("LUCRO", "YIELD"),
    ("FONÉTICA", "PHONETIC"),
    ("FONETICA", "PHONETIC"),
    ("FILTROXML", "FILTERXML"),
    ("SERVIÇOWEB", "WEBSERVICE"),
    ("SERVICOWEB", "WEBSERVICE"),
    ("MEMBROKPICUBO", "CUBEKPIMEMBER"),
    ("MEMBROCUBO", "CUBEMEMBER"),
    ("PROPRIEDADEMEMBROCUBO", "CUBEMEMBERPROPERTY"),
    ("MEMBROCLASSIFICADOCUBO", "CUBERANKEDMEMBER"),
    ("CONJUNTOCUBO", "CUBESET"),
    ("CONTAGEMCONJUNTOCUBO", "CUBESETCOUNT"),
    ("VALORCUBO", "CUBEVALUE"),
    ("CHAMAR", "CALL"),
    ("REGISTRO.ID", "REGISTER.ID"),
    ("HISTÓRICODEAÇÕES", "STOCKHISTORY"),
    ("HISTORICODEACOES", "STOCKHISTORY"),
    ("TRADUZIR", "TRANSLATE"),
    ("DETECTARIDIOMA", "DETECTLANGUAGE"),
    ("SOMA", "SUM"),
    ("AGREGAR", "AGGREGATE"),
    ("MÉDIA", "AVERAGE"),
    ("MEDIA", "AVERAGE"),
    ("MED", "MEDIAN"),
    ("SE", "IF"),
    ("SES", "IFS"),
    ("SEERRO", "IFERROR"),
    ("E", "AND"),
    ("OU", "OR"),
    ("NÃO", "NOT"),
    ("NAO", "NOT"),
    ("CONT.NÚM", "COUNT"),
    ("CONT.NUM", "COUNT"),
    ("CONT.VALORES", "COUNTA"),
    ("CONT.SE", "COUNTIF"),
    ("CONTSE", "COUNTIF"),
    ("SOMASE", "SUMIF"),
    ("SOMARPRODUTO", "SUMPRODUCT"),
    ("MÉDIASE", "AVERAGEIF"),
    ("MEDIASE", "AVERAGEIF"),
    ("MÁXIMO", "MAX"),
    ("MAXIMO", "MAX"),
    ("MÍNIMO", "MIN"),
    ("MINIMO", "MIN"),
    ("ARRED", "ROUND"),
    ("ARREDONDAR.PARA.CIMA", "ROUNDUP"),
    ("ARREDONDAR.PARA.BAIXO", "ROUNDDOWN"),
    ("RAIZ", "SQRT"),
    ("POTÊNCIA", "POWER"),
    ("POTENCIA", "POWER"),
    ("RESTO", "MOD"),
    ("ABS", "ABS"),
    ("INT", "INT"),
    ("ALEATÓRIO", "RAND"),
    ("ALEATORIO", "RAND"),
    ("ALEATÓRIOENTRE", "RANDBETWEEN"),
    ("IMAGEM", "IMAGE"),
    ("IMAGEM.BASE64", "IMAGE.BASE64"),
    ("MAIÚSCULA", "UPPER"),
    ("MAIUSCULA", "UPPER"),
    ("MINÚSCULA", "LOWER"),
    ("MINUSCULA", "LOWER"),
    ("PRI.MAIÚSCULA", "PROPER"),
    ("ARRUMAR", "TRIM"),
    ("ESQUERDA", "LEFT"),
    ("DIREITA", "RIGHT"),
    ("EXT", "MID"),
    ("EXT.TEXTO", "MID"),
    ("NÚM.CARACT", "LEN"),
    ("NUM.CARACT", "LEN"),
    ("CONCATENAR", "CONCATENATE"),
    ("SUBSTITUIR", "SUBSTITUTE"),
    ("CARACT", "CHAR"),
    ("CÓDIGO", "CODE"),
    ("CODIGO", "CODE"),
    ("TIRAR", "CLEAN"),
    ("EXATO", "EXACT"),
    ("MUDAR", "REPLACE"),
    ("MOEDA", "DOLLAR"),
    ("DEF.NÚM.DEC", "FIXED"),
    ("DEF.NUM.DEC", "FIXED"),
    ("VALORNUMÉRICO", "NUMBERVALUE"),
    ("VALORNUMERICO", "NUMBERVALUE"),
    ("CARACTUNICODE", "UNICHAR"),
    ("TEXTOANTES", "TEXTBEFORE"),
    ("TEXTODEPOIS", "TEXTAFTER"),
    ("VALORPARATEXTO", "VALUETOTEXT"),
    ("MATRIZPARATEXTO", "ARRAYTOTEXT"),
    ("REGEXTESTE", "REGEXTEST"),
    ("PROCURAR", "FIND"),
    ("LOCALIZAR", "SEARCH"),
    ("REPETIR", "REPT"),
    ("TEXTO", "TEXT"),
    ("VALOR", "VALUE"),
    ("PROCV", "VLOOKUP"),
    ("PROCH", "HLOOKUP"),
    ("PROCX", "XLOOKUP"),
    ("FILTRO", "FILTER"),
    ("ÚNICO", "UNIQUE"),
    ("UNICO", "UNIQUE"),
    ("CLASSIFICAR", "SORT"),
    ("ORDENAR", "SORT"),
    ("TRANSPOR", "TRANSPOSE"),
    ("MATRIZ.TRANSPOSTA", "TRANSPOSE"),
    ("MATRIZ.MULT", "MMULT"),
    ("MATRIZ.MULTIPLICAR", "MMULT"),
    ("MATRIZ.INVERSO", "MINVERSE"),
    ("MATRIZ.DETERM", "MDETERM"),
    ("SEQUÊNCIA", "SEQUENCE"),
    ("SEQUENCIA", "SEQUENCE"),
    ("ÍNDICE", "INDEX"),
    ("INDICE", "INDEX"),
    ("CORRESP", "MATCH"),
    ("ESCOLHER", "CHOOSE"),
    ("LIN", "ROW"),
    ("COL", "COLUMN"),
    ("LINS", "ROWS"),
    ("COLS", "COLUMNS"),
    ("FREQÜÊNCIA", "FREQUENCY"),
    ("FREQUÊNCIA", "FREQUENCY"),
    ("FREQUENCIA", "FREQUENCY"),
    ("HOJE", "TODAY"),
    ("AGORA", "NOW"),
    ("DATA", "DATE"),
    ("DATADIF", "DATEDIF"),
    ("DATEDIFF", "DATEDIF"),
    ("DATA.DIF", "DATEDIF"),
    ("ANO", "YEAR"),
    ("MÊS", "MONTH"),
    ("MES", "MONTH"),
    ("DIA", "DAY"),
    ("UNIRTEXTO", "TEXTJOIN"),
    ("DIVIDIRTEXTO", "TEXTSPLIT"),
    ("JUNTAR", "JOIN"),
    ("REGEXEXTRAIR", "REGEXEXTRACT"),
    ("REGEXCORRESP", "REGEXMATCH"),
    ("REGEXSUBSTITUIR", "REGEXREPLACE"),
    ("FIMMÊS", "EOMONTH"),
    ("FIMMES", "EOMONTH"),
    ("DATAM", "EDATE"),
    ("DIATRABALHOTOTAL", "NETWORKDAYS"),
    ("DIATRABALHO", "WORKDAY"),
    ("NÚMSEMANA", "WEEKNUM"),
    ("NUMSEMANA", "WEEKNUM"),
    ("PARÂMETRO", "SWITCH"),
    ("PARAMETRO", "SWITCH"),
    ("SENÃODISP", "IFNA"),
    ("SENAODISP", "IFNA"),
    // NÃO.DISP é o nome do Excel em português; sem ponto fica como apelido.
    ("NÃO.DISP", "NA"),
    ("NAO.DISP", "NA"),
    ("NÃODISP", "NA"),
    ("NAODISP", "NA"),
    ("NA", "NA"),
    ("DEIXAR", "LET"),
    ("MAPEAR", "MAP"),
    ("REDUZIR", "REDUCE"),
    ("DESVPAD", "STDEV"),
    ("SOMASES", "SUMIFS"),
    ("CONT.SES", "COUNTIFS"),
    ("CONTSES", "COUNTIFS"),
    ("CONT.VAZIO", "COUNTBLANK"),
    ("CONTAR.VAZIO", "COUNTBLANK"),
    ("CONTVAZIO", "COUNTBLANK"),
    ("MÉDIASES", "AVERAGEIFS"),
    ("MEDIASES", "AVERAGEIFS"),
    ("MÁXIMOSES", "MAXIFS"),
    ("MAXIMOSES", "MAXIFS"),
    ("MÍNIMOSES", "MINIFS"),
    ("MINIMOSES", "MINIFS"),
    ("MODO", "MODE"),
    ("MAIOR", "LARGE"),
    ("MENOR", "SMALL"),
    ("PERCENTIL", "PERCENTILE"),
    ("QUARTIL", "QUARTILE"),
    ("ORDEM", "RANK"),
    ("DESV.MÉDIO", "AVEDEV"),
    ("DESVQ", "DEVSQ"),
    ("MÉDIA.GEOMÉTRICA", "GEOMEAN"),
    ("MÉDIA.HARMÔNICA", "HARMEAN"),
    ("DISTORÇÃO", "SKEW"),
    ("CURT", "KURT"),
    ("CONT.ÚNICO", "COUNTUNIQUE"),
    ("CONTAR.ÚNICO", "COUNTUNIQUE"),
    ("CONTAR.UNICO", "COUNTUNIQUE"),
    ("SEN", "SIN"),
    ("SINAL", "SIGN"),
    ("ASEN", "ASIN"),
    ("TG", "TAN"),
    ("ATG", "ATAN"),
    ("SENH", "SINH"),
    ("TGH", "TANH"),
    ("RADIANOS", "RADIANS"),
    ("GRAUS", "DEGREES"),
    ("CORREL", "CORREL"),
    ("COVAR", "COVAR"),
    ("INCLINAÇÃO", "SLOPE"),
    ("INCLINACAO", "SLOPE"),
    ("INTERCEPÇÃO", "INTERCEPT"),
    ("INTERCEPCAO", "INTERCEPT"),
    ("RQUAD", "RSQ"),
    ("PREVISÃO", "FORECAST"),
    ("PREVISAO", "FORECAST"),
    ("FATORIAL", "FACT"),
    ("HORA", "HOUR"),
    ("MINUTO", "MINUTE"),
    ("SEGUNDO", "SECOND"),
    ("TEMPO", "TIME"),
    ("DATA.VALOR", "DATEVALUE"),
    ("VALOR.TEMPO", "TIMEVALUE"),
    ("DIAS", "DAYS"),
    ("DIAS360", "DAYS360"),
    ("NÚMSEMANAISO", "ISOWEEKNUM"),
    ("NUMSEMANAISO", "ISOWEEKNUM"),
    ("FRAÇÃOANO", "YEARFRAC"),
    ("FRACAOANO", "YEARFRAC"),
    ("DIATRABALHOTOTAL.INTL", "NETWORKDAYS.INTL"),
    ("DIATRABALHO.INTL", "WORKDAY.INTL"),
    ("PAR", "EVEN"),
    // Engenharia (nomes oficiais do Excel em português).
    ("BINADEC", "BIN2DEC"),
    ("BINAHEX", "BIN2HEX"),
    ("BINAOCT", "BIN2OCT"),
    ("DECABIN", "DEC2BIN"),
    ("DECAHEX", "DEC2HEX"),
    ("DECAOCT", "DEC2OCT"),
    ("HEXABIN", "HEX2BIN"),
    ("HEXADEC", "HEX2DEC"),
    ("HEXAOCT", "HEX2OCT"),
    ("OCTABIN", "OCT2BIN"),
    ("OCTADEC", "OCT2DEC"),
    ("OCTAHEX", "OCT2HEX"),
    ("DESLOCESQBIT", "BITLSHIFT"),
    ("DESLOCDIRBIT", "BITRSHIFT"),
    ("DEGRAU", "GESTEP"),
    ("ÍMPAR", "ODD"),
    ("IMPAR", "ODD"),
    ("MDC", "GCD"),
    ("MMC", "LCM"),
    ("MARRED", "MROUND"),
    ("QUOCIENTE", "QUOTIENT"),
    ("SOMAQUAD", "SUMSQ"),
    ("RAIZPI", "SQRTPI"),
    ("FATDUPLO", "FACTDOUBLE"),
    ("ROMANO", "ROMAN"),
    ("ASENH", "ASINH"),
    ("ARÁBICO", "ARABIC"),
    ("ARABICO", "ARABIC"),
    ("TETO.MAT", "CEILING.MATH"),
    ("TETO.PRECISO", "CEILING.PRECISE"),
    ("ISO.TETO", "ISO.CEILING"),
    ("ARREDMULTB.MAT", "FLOOR.MATH"),
    ("ARREDMULTB.PRECISO", "FLOOR.PRECISE"),
    ("SOMASEQUÊNCIA", "SERIESSUM"),
    ("SOMASEQUENCIA", "SERIESSUM"),
    ("SOMAX2DY2", "SUMX2MY2"),
    ("SOMAX2SY2", "SUMX2PY2"),
    ("SOMAXMY2", "SUMXMY2"),
    ("COMBIN", "COMBIN"),
    ("COMBINAÇÃO", "COMBIN"),
    ("PERMUTAÇÃO", "PERMUT"),
    ("PERMUTAS", "PERMUTATIONA"),
    ("DIST.NORM", "NORMDIST"),
    ("DISTNORM", "NORMDIST"),
    ("DIST.NORM.N", "NORM.DIST"),
    ("DIST.NORMP", "NORMSDIST"),
    ("DISTNORMP", "NORMSDIST"),
    ("DIST.NORMP.N", "NORM.S.DIST"),
    ("INV.NORM", "NORMINV"),
    ("INVNORM", "NORMINV"),
    ("INV.NORM.N", "NORM.INV"),
    ("DIST.T", "T.DIST"),
    ("DISTT", "TDIST"),
    ("TESTE.T", "T.TEST"),
    ("TESTET", "T.TEST"),
    ("DIST.QUI", "CHIDIST"),
    ("DISTQUI", "CHIDIST"),
    ("DIST.QUIQUA", "CHISQ.DIST"),
    ("TESTE.QUI", "CHISQ.TEST"),
    ("TESTEQUI", "CHISQ.TEST"),
    ("CHITEST", "CHISQ.TEST"),
    ("TTEST", "T.TEST"),
    // Engenharia: nomes em português dos complexos (IM*).
    ("COMPLEXO", "COMPLEX"),
    ("IMAGINÁRIO", "IMAGINARY"),
    ("IMAGINARIO", "IMAGINARY"),
    ("IMARG", "IMARGUMENT"),
    ("IMCONJ", "IMCONJUGATE"),
    ("IMCOSEC", "IMCSC"),
    ("IMCOSECH", "IMCSCH"),
    ("IMPOT", "IMPOWER"),
    ("IMPROD", "IMPRODUCT"),
    ("IMSENO", "IMSIN"),
    ("IMSENH", "IMSINH"),
    ("IMRAIZ", "IMSQRT"),
    ("IMSUBTR", "IMSUB"),
    ("IMSOMA", "IMSUM"),
    ("DIST.BETA.N", "BETA.DIST"),
    ("INV.BETA.N", "BETA.INV"),
    ("DISTBETA", "BETADIST"),
    ("INVBETA", "BETAINV"),
    ("DISTR.BINOM.N", "BINOM.DIST"),
    ("DIST.BINOM.INTERVALO", "BINOM.DIST.RANGE"),
    ("INV.BINOM", "BINOM.INV"),
    ("DISTRBINOM", "BINOMDIST"),
    ("CRIT.BINOM", "CRITBINOM"),
    ("DIST.QUIQUA.CD", "CHISQ.DIST.RT"),
    ("INV.QUIQUA", "CHISQ.INV"),
    ("INV.QUIQUA.CD", "CHISQ.INV.RT"),
    ("INV.QUI", "CHIINV"),
    ("DIST.EXPON", "EXPON.DIST"),
    ("DISTEXPON", "EXPONDIST"),
    ("DIST.F", "F.DIST"),
    ("DIST.F.CD", "F.DIST.RT"),
    ("INV.F", "F.INV"),
    ("INV.F.CD", "F.INV.RT"),
    ("DISTF", "FDIST"),
    ("INVF", "FINV"),
    ("GAMA", "GAMMA"),
    ("DIST.GAMA", "GAMMA.DIST"),
    ("INV.GAMA", "GAMMA.INV"),
    ("DISTGAMA", "GAMMADIST"),
    ("INVGAMA", "GAMMAINV"),
    ("LNGAMA", "GAMMALN"),
    ("LNGAMA.PRECISO", "GAMMALN.PRECISE"),
    ("DIST.HIPERGEOM.N", "HYPGEOM.DIST"),
    ("DIST.HIPERGEOM", "HYPGEOMDIST"),
    ("DIST.LOGNORMAL.N", "LOGNORM.DIST"),
    ("INV.LOGNORMAL.N", "LOGNORM.INV"),
    ("DIST.LOGNORMAL", "LOGNORMDIST"),
    ("INV.LOG", "LOGINV"),
    ("DIST.BIN.NEG.N", "NEGBINOM.DIST"),
    ("DIST.BIN.NEG", "NEGBINOMDIST"),
    ("INV.NORMP.N", "NORM.S.INV"),
    ("INV.NORMP", "NORMSINV"),
    ("DIST.POISSON", "POISSON.DIST"),
    ("DIST.T.BC", "T.DIST.2T"),
    ("DIST.T.CD", "T.DIST.RT"),
    ("INV.T", "T.INV"),
    ("INV.T.BC", "T.INV.2T"),
    ("INVT", "TINV"),
    ("DIST.WEIBULL", "WEIBULL.DIST"),
    ("INT.CONFIANÇA.NORM", "CONFIDENCE.NORM"),
    ("INT.CONFIANCA.NORM", "CONFIDENCE.NORM"),
    ("INT.CONFIANÇA.T", "CONFIDENCE.T"),
    ("INT.CONFIANCA.T", "CONFIDENCE.T"),
    ("INT.CONFIANÇA", "CONFIDENCE"),
    ("INT.CONFIANCA", "CONFIDENCE"),
    ("FUNERRO", "ERF"),
    ("FUNERRO.PRECISO", "ERF.PRECISE"),
    ("FUNERROCOMPL", "ERFC"),
    ("FUNERROCOMPL.PRECISO", "ERFC.PRECISE"),
    ("DESVPADP", "STDEVP"),
    ("VERDADEIRO", "TRUE"),
    ("FALSO", "FALSE"),
    ("É.ERRO", "ISERROR"),
    ("ÉERRO", "ISERROR"),
    ("ÉCÉL.VAZIA", "ISBLANK"),
    ("ECEL.VAZIA", "ISBLANK"),
    ("ÉCÉL", "ISBLANK"),
    ("ÉNÚM", "ISNUMBER"),
    ("ÉTEXTO", "ISTEXT"),
    ("ÉPAR", "ISEVEN"),
    ("EPAR", "ISEVEN"),
    ("ÉÍMPAR", "ISODD"),
    ("ÉIMPAR", "ISODD"),
    ("EIMPAR", "ISODD"),
    ("ÉERR", "ISERR"),
    ("EERR", "ISERR"),
    ("É.NÃO.DISP", "ISNA"),
    ("ÉNÃO.DISP", "ISNA"),
    ("ENAO.DISP", "ISNA"),
    ("É.NAO.DISP", "ISNA"),
    ("ÉLÓGICO", "ISLOGICAL"),
    ("ÉLOGICO", "ISLOGICAL"),
    ("É.NÃO.TEXTO", "ISNONTEXT"),
    ("É.NAO.TEXTO", "ISNONTEXT"),
    ("TIPO.ERRO", "ERROR.TYPE"),
    ("TIPO", "TYPE"),
    // Fonte de dados
    ("IMPORTARDADOS", "IMPORTDATA"),
    ("IMPORTARJSON", "IMPORTJSON"),
    ("BUSCAR.TEXTO", "FETCHTEXT"),
    ("BUSCARTEXTO", "FETCHTEXT"),
    // Financeiras (fn_fin.rs)
    ("PGTO", "PMT"),
    ("VF", "FV"),
    ("VP", "PV"),
    ("IPGTO", "IPMT"),
    ("PPGTO", "PPMT"),
    ("ÉPGTO", "ISPMT"),
    ("EPGTO", "ISPMT"),
    ("DPD", "SLN"),
    ("SDA", "SYD"),
    ("BD", "DB"),
    ("BDD", "DDB"),
    ("BDV", "VDB"),
    ("EFETIVA", "EFFECT"),
    ("MOEDADEC", "DOLLARDE"),
    ("MOEDAFRA", "DOLLARFR"),
    ("TAXAJURO", "RRI"),
    ("DURAÇÃOP", "PDURATION"),
    ("DURACAOP", "PDURATION"),
    ("VPL", "NPV"),
    ("TAXA", "RATE"),
    ("PGTOJURACUM", "CUMIPMT"),
    ("PGTOCAPACUM", "CUMPRINC"),
    ("XVPL", "XNPV"),
    ("XTIR", "XIRR"),
    ("TIR", "IRR"),
    ("MTIR", "MIRR"),
    ("VFPLANO", "FVSCHEDULE"),
    // Banco de dados (fn_db.rs). Com e sem acento.
    ("BDSOMA", "DSUM"),
    ("BDMÉDIA", "DAVERAGE"),
    ("BDMEDIA", "DAVERAGE"),
    ("BDCONTAR", "DCOUNT"),
    ("BDCONTARA", "DCOUNTA"),
    ("BDEXTRAIR", "DGET"),
    ("BDMÁX", "DMAX"),
    ("BDMAX", "DMAX"),
    ("BDMÍN", "DMIN"),
    ("BDMIN", "DMIN"),
    ("BDMULTIPL", "DPRODUCT"),
    ("BDEST", "DSTDEV"),
    ("BDDESVPA", "DSTDEVP"),
    ("BDVAREST", "DVAR"),
    ("BDVARP", "DVARP"),
    // Estatística (fn_stats.rs). Com e sem acento.
    ("MÉDIAA", "AVERAGEA"),
    ("MEDIAA", "AVERAGEA"),
    ("MÁXIMOA", "MAXA"),
    ("MAXIMOA", "MAXA"),
    ("MÍNIMOA", "MINA"),
    ("MINIMOA", "MINA"),
    ("DESVPADA", "STDEVA"),
    ("DESVPADPA", "STDEVPA"),
    ("ORDEM.EQ", "RANK.EQ"),
    ("ORDEM.MÉD", "RANK.AVG"),
    ("ORDEM.MED", "RANK.AVG"),
    ("PERCENTIL.EXC", "PERCENTILE.EXC"),
    ("QUARTIL.EXC", "QUARTILE.EXC"),
    ("ORDEM.PORCENTUAL.INC", "PERCENTRANK.INC"),
    ("ORDEM.PORCENTUAL.EXC", "PERCENTRANK.EXC"),
    ("COVARIÂNCIA.P", "COVARIANCE.P"),
    ("COVARIANCIA.P", "COVARIANCE.P"),
    ("COVARIÂNCIA.S", "COVARIANCE.S"),
    ("COVARIANCIA.S", "COVARIANCE.S"),
    ("PADRONIZAR", "STANDARDIZE"),
    ("MÉDIA.INTERNA", "TRIMMEAN"),
    ("MEDIA.INTERNA", "TRIMMEAN"),
    ("PREVISÃO.LINEAR", "FORECAST.LINEAR"),
    ("PREVISAO.LINEAR", "FORECAST.LINEAR"),
    ("DISTORÇÃO.P", "SKEW.P"),
    ("DISTORCAO.P", "SKEW.P"),
    ("MODO.MULT", "MODE.MULT"),
    // Lote de 15/10/2026.
    ("PROC", "LOOKUP"),
    ("CORRESPX", "XMATCH"),
    ("ENDEREÇO", "ADDRESS"),
    ("ENDERECO", "ADDRESS"),
    ("EMPILHARV", "VSTACK"),
    ("EMPILHARH", "HSTACK"),
    ("PEGAR", "TAKE"),
    ("DESCARTAR", "DROP"),
    ("ESCOLHERCOLS", "CHOOSECOLS"),
    ("ESCOLHERLINS", "CHOOSEROWS"),
    ("PARACOL", "TOCOL"),
    ("PARALIN", "TOROW"),
    ("QUEBRARLINS", "WRAPROWS"),
    ("QUEBRARCOLS", "WRAPCOLS"),
    ("EXPANDIR", "EXPAND"),
    ("CLASSIFICARPOR", "SORTBY"),
    ("MATRIZALEATÓRIA", "RANDARRAY"),
    ("MATRIZALEATORIA", "RANDARRAY"),
    ("HIPERLINK", "HYPERLINK"),
    ("ÉREF", "ISREF"),
    ("EREF", "ISREF"),
    ("EPADYX", "STEYX"),
    ("TESTE.F", "F.TEST"),
    ("TESTEF", "F.TEST"),
    ("FTEST", "F.TEST"),
    ("TESTE.Z", "Z.TEST"),
    ("TESTEZ", "Z.TEST"),
    ("ZTEST", "Z.TEST"),
    ("TENDÊNCIA", "TREND"),
    ("TENDENCIA", "TREND"),
    ("CRESCIMENTO", "GROWTH"),
    ("NÚM.CARACTB", "LENB"),
    ("NUM.CARACTB", "LENB"),
    ("ESQUERDAB", "LEFTB"),
    ("DIREITAB", "RIGHTB"),
    ("EXT.TEXTOB", "MIDB"),
    ("PROCURARB", "FINDB"),
    ("LOCALIZARB", "SEARCHB"),
    ("MUDARB", "REPLACEB"),
    ("CODIFURL", "ENCODEURL"),
    ("CONVERTER", "CONVERT"),
    ("DESC", "DISC"),
    ("PREÇODESC", "PRICEDISC"),
    ("PRECODESC", "PRICEDISC"),
    ("LUCRODESC", "YIELDDISC"),
    ("TAXAJUROS", "INTRATE"),
    ("RECEBER", "RECEIVED"),
    ("OTNVALOR", "TBILLPRICE"),
    ("OTNLUCRO", "TBILLYIELD"),
    ("OTN", "TBILLEQ"),
    ("PREÇOVENC", "PRICEMAT"),
    ("PRECOVENC", "PRICEMAT"),
    ("LUCROVENC", "YIELDMAT"),
    ("JUROSACUMV", "ACCRINTM"),
    // Nomes do Excel em português que faltavam (16/10/2026): sem eles, a
    // interface em português mostrava o nome em inglês.
    ("TRUNCAR", "TRUNC"),
    ("MULT", "PRODUCT"),
    ("TETO", "CEILING"),
    ("ARREDMULTB", "FLOOR"),
    ("ORDEM.PORCENTUAL", "PERCENTRANK"),
    ("DESVPAD.P", "STDEV.P"),
    ("DESVPAD.A", "STDEV.S"),
    ("VAR.A", "VAR.S"),
    ("MODO.ÚNICO", "MODE.SNGL"),
    ("MODO.UNICO", "MODE.SNGL"),
    ("PERCENTIL.INC", "PERCENTILE.INC"),
    ("QUARTIL.INC", "QUARTILE.INC"),
    ("DIA.DA.SEMANA", "WEEKDAY"),
    ("OUEXCL", "XOR"),
    // Por cor (do FeatherSheet; o Excel não tem).
    ("SOMAR.POR.COR", "SUMBYCOLOR"),
    ("CONT.SE.COR", "COUNTBYCOLOR"),
];

/// Funções que devolvem matriz — o derramamento depende desta lista.
///
/// Quem acrescentar uma função de matriz **precisa** acrescentá-la aqui: sem
/// isso o armazenamento manda a célula para a coluna empacotada e o
/// derramamento se perde. Era a nota mais repetida do `CLAUDE.md`, e continua
/// valendo do lado Rust.
static DINAMICAS: &[&str] = &[
    "OFFSET",
    "INDIRECT",
    "TRIMRANGE",
    "BYROW",
    "BYCOL",
    "SCAN",
    "MAKEARRAY",
    "GROUPBY",
    "PIVOTBY",
    "LINEST",
    "LOGEST",
    "FILTERXML",
    "FILTER",
    "UNIQUE",
    "SORT",
    "TRANSPOSE",
    "SEQUENCE",
    "FREQUENCY",
    "MMULT",
    "MINVERSE",
    "MUNIT",
    // Estatística (fn_stats.rs): devolve a coluna de modas.
    "MODE.MULT",
    // Lote de 15/10/2026 (fn_extra.rs).
    "VSTACK",
    "HSTACK",
    "TAKE",
    "DROP",
    "CHOOSECOLS",
    "CHOOSEROWS",
    "TOCOL",
    "TOROW",
    "WRAPROWS",
    "WRAPCOLS",
    "EXPAND",
    "SORTBY",
    "RANDARRAY",
    "TREND",
    "GROWTH",
    // Leem de fora e derramam o que vier.
    "IMPORTDATA",
    "IMPORTJSON",
];

/// Funções que só viram matriz quando o argumento é matriz.
static LEVANTAM_MATRIZ: &[&str] =
    &["IF", "YEAR", "MONTH", "DAY", "WEEKDAY", "LARGE", "SMALL"];

/// Que cara tem o resultado de uma função que devolve data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateResult {
    Date,
    DateTime,
}

/// A implementação de uma função, pelo nome em inglês ou em português.
pub fn lookup(nome: &str) -> Option<FnImpl> {
    #[cfg(test)]
    if let Some(f) = teste::implementada(&nome.to_uppercase()) {
        return Some(f);
    }
    let mapa = mapa_de_funcoes();
    mapa.get(nome).or_else(|| mapa.get(nome.to_uppercase().as_str())).copied()
}

/// Nome em inglês ou apelido em português (maiúsculas) → implementação,
/// montado uma vez. É chamado a cada célula avaliada: a busca linear em
/// `FUNCOES` e `ALIASES` (com uma maiúscula alocada) era 14% de abrir uma
/// planilha de 1 milhão de linhas (28/10/2026). O nome exato ganha do apelido.
fn mapa_de_funcoes() -> &'static std::collections::HashMap<&'static str, FnImpl> {
    static MAPA: std::sync::OnceLock<std::collections::HashMap<&'static str, FnImpl>> = std::sync::OnceLock::new();
    MAPA.get_or_init(|| {
        let mut mapa = std::collections::HashMap::new();
        for (nome, f) in FUNCOES {
            mapa.entry(*nome).or_insert(*f);
        }
        for (apelido, canonico) in ALIASES {
            if let Some(f) = FUNCOES.iter().find(|(n, _)| n == canonico).map(|(_, f)| *f) {
                mapa.entry(*apelido).or_insert(f);
            }
        }
        mapa
    })
}

fn implementada(nome: &str) -> Option<FnImpl> {
    #[cfg(test)]
    if let Some(f) = teste::implementada(nome) {
        return Some(f);
    }
    FUNCOES
        .iter()
        .find(|(n, _)| *n == nome)
        .map(|(_, f)| *f)
}

/// O nome canônico em inglês de um apelido em português.
///
/// Responde pela tabela de apelidos mesmo quando a função ainda não foi
/// portada — o `CANONICAL` do TypeScript só registrava o que já existia, mas
/// ele era usado para montar conjuntos derivados, e nenhum deles muda por
/// causa disso. Enquanto a migração corre, responder sempre é o que mantém a
/// ajuda e o catálogo funcionando.
pub fn canonical(nome: &str) -> Option<&'static str> {
    let maiusculo = nome.to_uppercase();
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == maiusculo)
        .map(|(_, real)| *real)
}

/// A tabela inteira de apelidos (português -> inglês), para o catálogo.
pub fn aliases() -> &'static [(&'static str, &'static str)] {
    ALIASES
}

/// Se a função devolve matriz (aceita apelido).
pub fn is_dynamic(nome: &str) -> bool {
    na_lista(DINAMICAS, nome)
}

/// Se a função vira matriz quando o argumento é matriz (aceita apelido).
pub fn array_lifting(nome: &str) -> bool {
    na_lista(LEVANTAM_MATRIZ, nome)
}

/// Se o resultado da função é uma data, e de que tipo (aceita apelido).
pub fn date_result(nome: &str) -> Option<DateResult> {
    let canonico = nome_canonico(nome);
    match canonico.as_str() {
        "TODAY" | "DATE" => Some(DateResult::Date),
        "NOW" => Some(DateResult::DateTime),
        _ => None,
    }
}

fn na_lista(lista: &[&str], nome: &str) -> bool {
    lista.contains(&nome_canonico(nome).as_str())
}

fn nome_canonico(nome: &str) -> String {
    let maiusculo = nome.to_uppercase();
    match canonical(&maiusculo) {
        Some(c) => c.to_string(),
        None => maiusculo,
    }
}

/// `LET`, `LAMBDA`, `MAP` e `REDUCE`: o `evaluator.rs` as atende pelo nome,
/// fora da tabela `FUNCOES`.
const FORMAS_ESPECIAIS: [&str; 4] = ["LAMBDA", "LET", "MAP", "REDUCE"];

/// Todos os nomes que a pessoa pode digitar, em ordem — é o que alimenta o
/// autocompletar da barra de fórmulas.
pub fn function_names() -> Vec<&'static str> {
    let mut nomes: Vec<&'static str> = FUNCOES.iter().map(|(n, _)| *n).collect();
    // As formas especiais que o avaliador resolve antes da tabela (o nome
    // delas define variável, e não é argumento comum): também se digitam.
    nomes.extend(FORMAS_ESPECIAIS);
    let existe = |nome: &str| implementada(nome).is_some() || FORMAS_ESPECIAIS.contains(&nome);
    for (alias, real) in ALIASES {
        if existe(real) && !existe(alias) {
            nomes.push(alias);
        }
    }
    nomes.sort_unstable();
    nomes.dedup();
    nomes
}

/// Funções de mentira, só para os testes do avaliador e do registro terem o
/// que chamar enquanto as famílias de verdade não chegam.
#[cfg(test)]
pub(crate) mod teste {
    use super::FnImpl;
    use crate::formula::ast::Node;
    use crate::formula::evaluator::Evaluator;
    use crate::celulas::values::{Computed, Matrix, Value};

    fn dobro(args: &[Node], ev: &Evaluator<'_>) -> Computed {
        match ev.number(&args[0]) {
            Ok(n) => Computed::Scalar(Value::Number(n * 2.0)),
            Err(e) => Computed::Scalar(Value::Error(e)),
        }
    }

    /// Devolve sempre uma matriz 2×1 — serve para testar o derramamento.
    fn duas_linhas(_args: &[Node], _ev: &Evaluator<'_>) -> Computed {
        Computed::Matrix(Matrix::new(vec![
            vec![Value::Number(1.0)],
            vec![Value::Number(2.0)],
        ]))
    }

    /// Não avalia o argumento: prova que a função recebe a árvore, não o
    /// valor.
    fn conta_argumentos(args: &[Node], _ev: &Evaluator<'_>) -> Computed {
        Computed::Scalar(Value::Number(args.len() as f64))
    }

    static TABELA: &[(&str, FnImpl)] = &[
        ("DOBRO", dobro as FnImpl),
        ("DUASLINHAS", duas_linhas as FnImpl),
        ("QUANTOS", conta_argumentos as FnImpl),
    ];

    pub(crate) fn implementada(nome: &str) -> Option<FnImpl> {
        TABELA.iter().find(|(n, _)| *n == nome).map(|(_, f)| *f)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::formula::evaluator::{planilha_de_mentira::Fake, Evaluator};
    use crate::formula::parser::parse_formula;
    use crate::celulas::values::Value;

    #[test]
    fn mapa_de_nomes_responde_como_a_varredura() {
        // A busca de antes do mapa: nome exato, senão o apelido.
        let varrer = |nome: &str| {
            let m = nome.to_uppercase();
            implementada(&m).or_else(|| implementada(canonical(&m)?))
        };
        let ptr = |f: Option<FnImpl>| f.map(|f| f as usize);
        for nome in FUNCOES.iter().map(|(n, _)| *n).chain(ALIASES.iter().map(|(a, _)| *a)) {
            for escrito in [nome.to_string(), nome.to_lowercase()] {
                assert_eq!(ptr(lookup(&escrito)), ptr(varrer(&escrito)), "{escrito}");
            }
        }
        assert!(lookup("NAO.EXISTE").is_none());
    }

    #[test]
    fn apelido_resolve_para_o_nome_canonico() {
        assert_eq!(canonical("SOMA"), Some("SUM"));
        assert_eq!(canonical("soma"), Some("SUM"));
        assert_eq!(canonical("MÉDIA"), Some("AVERAGE"));
        assert_eq!(canonical("MEDIA"), Some("AVERAGE"));
        assert_eq!(canonical("SUM"), None, "o nome canônico não é apelido de nada");
        assert_eq!(canonical("DIST.QUI"), Some("CHIDIST"), "cauda direita, dois argumentos");
        assert_eq!(canonical("MULT"), Some("PRODUCT"));
    }

    #[test]
    fn as_duas_grafias_do_acento_estao_na_tabela() {
        // A pessoa digita dos dois jeitos, e as duas têm de funcionar.
        for (com, sem) in [
            ("MÉDIA", "MEDIA"),
            ("NÃO", "NAO"),
            ("POTÊNCIA", "POTENCIA"),
            ("ÚNICO", "UNICO"),
            ("SEQUÊNCIA", "SEQUENCIA"),
            ("MÁXIMO", "MAXIMO"),
        ] {
            assert_eq!(canonical(com), canonical(sem), "{com} e {sem} divergem");
            assert!(canonical(com).is_some());
        }
    }

    #[test]
    fn nao_ha_apelido_repetido() {
        let mut vistos = std::collections::HashSet::new();
        for (alias, _) in ALIASES {
            assert!(vistos.insert(*alias), "apelido repetido: {alias}");
        }
    }

    #[test]
    fn funcao_de_matriz_e_reconhecida_pelos_dois_nomes() {
        assert!(is_dynamic("FILTER"));
        assert!(is_dynamic("FILTRO"));
        assert!(is_dynamic("único"));
        assert!(!is_dynamic("SUM"));
        // MDETERM devolve um número, não uma matriz.
        assert!(!is_dynamic("MATRIZ.DETERM"));
    }

    #[test]
    fn levantar_matriz_e_devolver_matriz_sao_coisas_diferentes() {
        assert!(array_lifting("SE"));
        assert!(array_lifting("IF"));
        assert!(!is_dynamic("SE"));
        assert!(array_lifting("ANO"));
    }

    #[test]
    fn resultado_de_data() {
        assert_eq!(date_result("HOJE"), Some(DateResult::Date));
        assert_eq!(date_result("TODAY"), Some(DateResult::Date));
        assert_eq!(date_result("AGORA"), Some(DateResult::DateTime));
        assert_eq!(date_result("SOMA"), None);
    }

    #[test]
    fn o_avaliador_chama_a_funcao_pelo_registro() {
        let f = Fake::default();
        let ev = Evaluator::new(&f);
        let v = ev.eval(&parse_formula("DOBRO(21)").unwrap());
        assert_eq!(v, Value::Number(42.0));
    }

    #[test]
    fn a_funcao_recebe_a_arvore_e_nao_o_valor() {
        // Se os argumentos chegassem avaliados, `1/0` teria virado erro antes
        // de a função decidir se queria olhar para ele.
        let f = Fake::default();
        let ev = Evaluator::new(&f);
        assert_eq!(
            ev.eval(&parse_formula("QUANTOS(1/0;2;3)").unwrap()),
            Value::Number(3.0)
        );
    }

    #[test]
    fn funcao_pode_devolver_matriz_para_derramar() {
        let f = Fake::default();
        let ev = Evaluator::new(&f);
        let r = ev.eval_array(&parse_formula("DUASLINHAS()").unwrap());
        assert_eq!(r.shape(), (2, 1));
        // E num lugar escalar sobra o primeiro valor.
        assert_eq!(ev.eval(&parse_formula("DUASLINHAS()").unwrap()), Value::Number(1.0));
    }
}
