\name{escape-equation-first-arg}
\title{Equation first argument escapes}
\description{\eqn{a\{b\}c \% d\\e
% equation percent
#ifdef EQUATION
literal conditional
#endif}{second}

\deqn{x\{y\}z \% w\\q
% deqn percent
#ifdef DEQN
literal conditional
#endif}{second}}
