\name{composite-dynamic-verbatim}
\title{Dynamic and verbatim content}
\RdOpts{stage=build,results=verbatim}
\description{Bare dynamic value: \Sexpr{1 + 1}.
Optioned dynamic value: \Sexpr[stage=render,results=rd]{2 + 2}.
\preformatted{
  literal \{ braces \}
}
\verb{literal \% percent}}
