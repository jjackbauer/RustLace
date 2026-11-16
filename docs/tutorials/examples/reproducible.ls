A = [[2, 1], [1, 3]]
b = [[5], [6]]
sol = matmul(inv(A), b)
residual = matmul(A, sol)
path = plot([1, 2, 3], [1, 4, 9], "squares")
det(A)
