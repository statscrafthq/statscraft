<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MLP

Multilayer perceptron: a neural network with one or two hidden layers that predicts scale dependents and classifies categorical ones.

## Syntax

```text
MLP dependent [(MLEVEL=S | O | N)] [dependent ...] [BY factors] [WITH covariates]
   [/EXCEPT VARIABLES=varlist]
   [/RESCALE [COVARIATE=STANDARDIZED | NORMALIZED | ADJNORMALIZED | NONE]
             [DEPENDENT=STANDARDIZED | NORMALIZED [(CORRECTION=0.02)] | ADJNORMALIZED [(CORRECTION=0.02)] | NONE]]
   [/PARTITION TRAINING=70 TESTING=30 HOLDOUT=0 | VARIABLE=var]
   [/ARCHITECTURE [AUTOMATIC=YES [(MINUNITS=1, MAXUNITS=50)] | NO]
                  [HIDDENLAYERS=1 | 2 [(NUMUNITS=AUTO | n [, n])]] [HIDDENFUNCTION=TANH | SIGMOID]
                  [OUTPUTFUNCTION=IDENTITY | SIGMOID | SOFTMAX | TANH]]
   [/CRITERIA [TRAINING=BATCH | ONLINE | MINIBATCH] [MINIBATCHSIZE=AUTO | n] [MEMSIZE=1000]
              [OPTIMIZATION=GRADIENTDESCENT | SCALEDCONJUGATE] [LEARNINGINITIAL=0.4]
              [LEARNINGLOWER=0.001] [MOMENTUM=0.9] [LEARNINGEPOCHS=10] [LAMBDAINITIAL=0.0000005]
              [SIGMAINITIAL=0.00005] [INTERVALCENTER=0] [INTERVALOFFSET=0.5]]
   [/STOPPINGRULES [ERRORSTEPS=1 [(DATA=AUTO | BOTH)]] [TRAININGTIMER=ON [(MAXTIME=15)] | OFF]
                   [MAXEPOCHS=AUTO | n] [ERRORCHANGE=0.0001] [ERRORRATIO=0.001]]
   [/MISSING USERMISSING=EXCLUDE | INCLUDE]
   [/PRINT [CPS] [NETWORKINFO] [SUMMARY] [CLASSIFICATION] [SOLUTION] [IMPORTANCE] [NONE]]
   [/PLOT [NETWORK] [PREDICTED] [RESIDUAL] [ROC] [GAIN] [LIFT] [NONE]]
   [/SAVE [PREDVAL[(names)]] [PSEUDOPROB[(root[:25] ...)]]] [/OUTFILE MODEL='file' ...].
```

Keywords are written in full and each subcommand comes once. A dependent is scale or categorical by MLEVEL, else by its measurement level; categorical dependents and factors get one unit per category of the training cases. Cases missing on any variable are left out (user-missing values of categorical variables count with INCLUDE); the rest go to training, testing and holdout at random by the shares (SET SEED makes it repeatable), or by the sign of the VARIABLE: positive trains, 0 tests, negative holds out. Covariates and scale dependents are rescaled by the training cases. Hidden units use TANH (default) or SIGMOID; the output units IDENTITY when a dependent is scale, else SOFTMAX with cross-entropy error; other outputs use the sum of squares. SIGMOID outputs need NORMALIZED dependents and TANH outputs ADJNORMALIZED, the defaults for them. AUTOMATIC=YES (default) uses one TANH layer and searches the number of units between MINUNITS and MAXUNITS on a sample of at most MEMSIZE (and 1000) cases, removing the weakest unit or adding one while the testing error drops.

NUMUNITS=AUTO starts from a count based on the numbers of input and output units and of training cases. Starting weights come from random draws in INTERVALCENTER +- INTERVALOFFSET, trained and shaken by simulated annealing on that sample. BATCH training uses SCALEDCONJUGATE by default; ONLINE and MINIBATCH use GRADIENTDESCENT on the cases in a random order, the learning rate falling from LEARNINGINITIAL to LEARNINGLOWER over LEARNINGEPOCHS passes. Training stops when the error (testing cases when there are any) has not reached a new low for ERRORSTEPS steps, at MAXTIME minutes, after MAXEPOCHS passes (AUTO: twice the weights plus 1, at least 100), or when the training error changes by less than ERRORCHANGE or falls below ERRORRATIO times that of predicting the means; the weights kept are those of the lowest error. Tables: Case Processing Summary (CPS), Network Information, Model Summary with the error, the relative error or percent incorrect and the rule that stopped training (SUMMARY), Classification per categorical dependent, Parameter Estimates (SOLUTION) and Independent Variable Importance with a chart (IMPORTANCE). Charts: the units of the network (NETWORK, default), predicted by observed (PREDICTED), residual by predicted (RESIDUAL), ROC with the areas, cumulative gains and lift of each category, from the training and testing cases. SAVE adds the predicted value or category (MLP\_PredictedValue) and the first n outputs of each categorical dependent (MLP\_PseudoProbability\_1, ...). OUTFILE writes Statscraft's own JSON model, one file per dependent, not with SPLIT FILE. The weight variable is not used.

## Example

```text
SET SEED=20251.
MLP churned (MLEVEL=N) BY plan region WITH tenure spend
  /PARTITION TRAINING=6 TESTING=2 HOLDOUT=2 /ARCHITECTURE AUTOMATIC=NO HIDDENLAYERS=1 (NUMUNITS=4)
  /PRINT CPS NETWORKINFO SUMMARY CLASSIFICATION IMPORTANCE /PLOT ROC /SAVE PREDVAL PSEUDOPROB.
```

`statscraft help MLP` prints the same text in the terminal. [All commands](README.md)
