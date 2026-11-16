A = [[2, 1], [1, 3]]
b = [[5], [6]]
Inv = inv(A)
sol = matmul(Inv, b)
residual = matmul(A, sol)
gap = residual - b
d = det(A)
residual
