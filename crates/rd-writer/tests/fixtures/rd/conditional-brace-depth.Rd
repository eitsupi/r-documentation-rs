\name{conditional-brace-depth}
\examples{
x <- function() {
#ifdef unix
}
#endif
y <- 1
}

\examples{
#ifdef unix
{
#endif
}
y <- 1
}
