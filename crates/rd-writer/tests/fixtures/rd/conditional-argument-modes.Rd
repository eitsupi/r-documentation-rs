\name{conditional-argument-modes}
\description{
before
#ifdef unix
inside latex
#ifndef windows
inside nested latex
#endif
after nested
#endif
after
  #ifdef unix
indented latex
#foo unknown latex
\preformatted{
#ifndef windows
inside verbatim
#endif
}
\eqn{
#ifdef unix
opaque equation
#endif
}{equation result}
}
\usage{
#ifdef unix
inside_usage()
#endif
}
\examples{
x <- "
#ifdef unix
inside_string()
#endif
"
x <- r"(
#ifdef unix
opaque raw
#endif
)"
}
