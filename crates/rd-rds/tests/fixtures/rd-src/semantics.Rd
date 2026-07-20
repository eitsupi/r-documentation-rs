\name{semantics}
\alias{semantics}
\title{Semantic view conformance fixture: \Sexpr{1 + 1}}
\RdOpts{stage=build,results=verbatim}
\description{
Links: \link{topic}, \link[pkg]{topic},
\link[pkg:topic]{display with \code{markup}}, and \link[=dest]{display}.
An external link is \href{https://example.org}{the site}.

Lifecycle: \ifelse{html}{\href{https://lifecycle.r-lib.org/articles/stages.html#stable}{\figure{lifecycle-stable.svg}{options: alt='[Stable]'}}}{\strong{[Stable]}}

\itemize{
  \item first item
  \item [label] body text beginning with brackets
  \item third item
}
\enumerate{
  \item first numbered item
  \item second numbered item
}
\describe{
  \item{first label}{first body}
  \item{second label}{second body with \code{markup}}
}

\tabular{lcr}{
  left \code{one} \tab center \emph{one} \tab right one \cr
  left two \tab \tab right \cr
  final left \tab final center \tab final right
}

Equations: \eqn{x^2}, \eqn{x^2}{x squared}, and
\deqn{\sum_i x_i}{sum x}.

Dynamic markup after defaults: \Sexpr[stage=render,results=rd]{2 + 2}.
}
