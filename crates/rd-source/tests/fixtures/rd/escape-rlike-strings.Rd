\name{escape-rlike-strings}
\title{R-like string escapes}
\examples{
f("in\{str\}")
s <- "p\\q"
t <- "a\{b\}c d\%e"
a \{ b \} c
r <- 1 \% 2
# cmt a\{b\} \% p\\q
z <- r"(raw \{b\} \% p\\q end)"
}
