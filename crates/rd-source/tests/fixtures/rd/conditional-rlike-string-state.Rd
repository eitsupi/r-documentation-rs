\name{conditional-rlike-string-state}
\examples{
"r1 open
#ifdef unix
a \emph{b} c
#endif
close"

"r2 open
#ifdef unix
body close"
#endif
\emph{after}

#ifdef unix
"r5 opens
still \emph{string}
#endif
close"

#ifdef unix
x <- "{
body {
} }"
#endif
}
